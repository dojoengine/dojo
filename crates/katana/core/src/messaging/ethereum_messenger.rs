use async_trait::async_trait;

use starknet::core::types::{MsgToL1, FieldElement};

use anyhow::Result;
use ethers::prelude::*;
use ethers::providers::{Http, Provider};
use ethers::types::{Address, BlockNumber, Log};
use ethers::providers::ProviderError;
use k256::ecdsa::SigningKey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use sha3::{Digest, Keccak256};

use starknet_api::core::{ContractAddress, Nonce, EntryPointSelector};
use starknet_api::hash::{self, StarkFelt};
use starknet_api::stark_felt;
use starknet_api::transaction::{
    Calldata, Fee, TransactionHash, L1HandlerTransaction as ApiL1HandlerTransaction, TransactionVersion,
};

use crate::messaging::{Messenger, MessengerError, MessengerResult};
use crate::sequencer::SequencerMessagingConfig;
use crate::backend::storage::transaction::{Transaction, L1HandlerTransaction};

abigen!(
    StarknetMessagingLocal,
    "./crates/katana/core/contracts/messaging/solidity/IStarknetMessagingLocal_ABI.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

///
#[derive(Debug, PartialEq, Eq, EthEvent)]
pub struct LogMessageToL2 {
    #[ethevent(indexed)]
    from_address: Address,
    #[ethevent(indexed)]
    to_address: U256,
    #[ethevent(indexed)]
    selector: U256,
    payload: Vec<U256>,
    nonce: U256,
    fee: U256,
}

///
pub struct EthereumMessenger {
    provider: Provider<Http>,
    provider_signer: Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    messaging_contract_address: Address,
}

impl EthereumMessenger {
    pub async fn new(config: SequencerMessagingConfig) -> Result<EthereumMessenger> {
        let provider = Provider::<Http>::try_from(&config.rpc_url)?;

        let chain_id = provider.get_chainid().await?;

        let wallet: LocalWallet = config.private_key
            .parse::<LocalWallet>()?
            .with_chain_id(chain_id.as_u32());

        let provider_signer = SignerMiddleware::new(provider.clone(), wallet.clone());
        let messaging_contract_address = Address::from_str(&config.contract_address)?;

        Ok(EthereumMessenger {
            provider,
            provider_signer: Arc::new(provider_signer),
            messaging_contract_address,
        })
    }

    /// Fetches logs in given block range and returns
    /// a HashMap with logs vector for each block.
    ///
    /// There is not pagination in ethereum, and no hard limit on block range.
    /// Fetching too much block may result in RPC request error.
    /// For this reason, the caller may wisely choose the range.
    ///
    /// # Arguments
    ///
    /// * `from_block` - The first block of which logs must be fetched.
    /// * `to_block` - The last block of which logs must be fetched.
    pub async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> MessengerResult<HashMap<u64, Vec<Log>>> {
        tracing::trace!("Fetching blocks {} - {}.", from_block, to_block);

        let mut logs: HashMap<u64, Vec<Log>> = HashMap::new();

        let filters = Filter {
            block_option: FilterBlockOption::Range {
                from_block: Some(BlockNumber::Number(from_block.into())),
                to_block: Some(BlockNumber::Number(to_block.into())),
            },
            address: Some(ValueOrArray::Value(self.messaging_contract_address)),

            // TODO: the topic is needed! To only gather message logs.
            topics: Default::default(),
        };

        self.provider.get_logs(&filters).await?
            .iter()
            .filter(|&l| l.block_number.is_some())
            .for_each(|l| {

                logs.entry(l.block_number
                           .unwrap() // safe as we filter on Some only.
                           .try_into()
                           .expect("Block number couldn't be converted to u64."))
                    .and_modify(|v| v.push(l.clone()))
                    .or_insert(vec![l.clone()]);
            });

        Ok(logs)
    }
}

#[async_trait]
impl Messenger for EthereumMessenger {
    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64
    ) -> MessengerResult<(u64, Vec<Transaction>)> {
        let chain_latest_block: u64 = self.provider.get_block_number().await?
            .try_into()
            .expect("Can't convert latest block number into u64.");

        // +1 as the from_block counts as 1 block fetched.
        let to_block = if from_block + max_blocks + 1 < chain_latest_block {
            from_block + max_blocks
        } else {
            chain_latest_block
        };

        let mut l1_handler_txs = vec![];

        self.fetch_logs(from_block, to_block).await?
            .iter()
            .for_each(|(block_number, block_logs)| {
                tracing::debug!(
                    "Converting logs of block {} into L1HandlerTx ({} logs)",
                    block_number,
                    block_logs.len(),
                );

                block_logs
                    .iter()
                    .for_each(|l| {
                        match l1_handler_tx_from_log(l) {
                            Ok(tx) => l1_handler_txs.push(tx),
                            // TODO: Some logs are just not to be considered.
                            // So, we can accept a list of topics to filter the logs,
                            // which could be the best option.
                            Err(_) => (),
                            
                        }
                    })
            });

        Ok((to_block, l1_handler_txs))
    }

    async fn settle_messages(&self, messages: &Vec<MsgToL1>) -> MessengerResult<()> {
        if messages.len() == 0 {
            return Ok(());
        }

        let starknet_messaging = StarknetMessagingLocal::new(
            self.messaging_contract_address,
            self.provider_signer.clone()
        );

        let mut hashes: Vec<U256> = vec![];

        for m in messages {
            let mut buf: Vec<u8> = vec![];
            buf.extend(m.from_address.to_bytes_be());
            buf.extend(m.to_address.to_bytes_be());
            buf.extend(FieldElement::from(m.payload.len()).to_bytes_be());
            for p in &m.payload {
                buf.extend(p.to_bytes_be());
            }

            hashes.push(compute_message_hash(&buf));
        }

        tracing::debug!("Sending transaction on L1 to register messages...");
        // TODO: add more info about the error.
        match starknet_messaging.add_message_hashes_from_l2(hashes.clone())
            .send()
            .await.map_err(|_| MessengerError::SendError)
            .unwrap()
            .await?
        {
            Some(receipt) => {
                trace_hashes(receipt.transaction_hash, hashes);
                Ok(())
            }
            None => {
                tracing::warn!("No receipt for L1 transaction.");
                Err(MessengerError::SendError)
            }
        }
    }

    async fn execute_messages(&self, _messages: &Vec<MsgToL1>) -> MessengerResult<()> {
        Ok(())
    }
}

///
fn trace_hashes(tx_hash: TxHash, hashes: Vec<U256>) {
    tracing::trace!(
        "Transaction on L1 for {} messages: {:#x}",
        hashes.len(),
        tx_hash,
    );

    for h in hashes {
        tracing::trace!("Msg hash sent=[{:#x}]", h);
    }
}

/// Computes the message hash.
fn compute_message_hash(data: &[u8]) -> U256 {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let hash_bytes = hash.as_slice();
    U256::from_big_endian(&hash_bytes)
}

/// Converts a starknet core log into a L1 handler transaction.
fn l1_handler_tx_from_log(log: &Log) -> Result<Transaction> {
    let parsed_log = <LogMessageToL2 as EthLogDecode>::decode_log(&log.clone().into())?;

    let from_address = stark_felt_from_address(parsed_log.from_address);
    let contract_address = stark_felt_from_u256(parsed_log.to_address);
    let selector = stark_felt_from_u256(parsed_log.selector);
    let nonce = stark_felt_from_u256(parsed_log.nonce);
    let fee: u128 = parsed_log.fee
        .try_into()
        .expect("Fee does not fit into u128.");

    let mut calldata_vec = vec![from_address];
    for p in parsed_log.payload {
        calldata_vec.push(stark_felt_from_u256(p));
    }

    let calldata = Calldata(calldata_vec.into());

    // TODO: not sure about how this must be computed,
    // at least with a nonce + address we should be
    // ok for now? Or is it derived from something?
    let tx_hash = hash::pedersen_hash(&nonce, &contract_address);

    let tx = Transaction::L1Handler(L1HandlerTransaction {
        inner: ApiL1HandlerTransaction {
            transaction_hash: TransactionHash(tx_hash),
            version: TransactionVersion(stark_felt!(1 as u32)),
            nonce: Nonce(nonce),
            contract_address: ContractAddress::try_from(contract_address).unwrap(),
            entry_point_selector: EntryPointSelector(selector),
            calldata: calldata,
        },
        paid_fee_on_l1: Fee(fee),
    });

    Ok(tx)
}

fn stark_felt_from_u256(v: U256) -> StarkFelt {
    stark_felt!(format!("{:#064x}", v).as_str())
}

fn stark_felt_from_address(v: Address) -> StarkFelt {
    stark_felt!(format!("{:#064x}", v).as_str())
}

impl From<ProviderError> for MessengerError {
    fn from(e: ProviderError) -> MessengerError {
        MessengerError::EthereumProviderError(e)
    }
}
