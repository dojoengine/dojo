use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use ethers::prelude::*;
use ethers::providers::{Http, Provider};
use ethers::types::{Address, BlockNumber, Log};
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};
use starknet::core::types::{FieldElement, MsgToL1};
use starknet_api::core::{ContractAddress, EntryPointSelector, Nonce};
use starknet_api::hash::{self, StarkFelt};
use starknet_api::stark_felt;
use starknet_api::transaction::{
    Calldata, L1HandlerTransaction as ApiL1HandlerTransaction, TransactionHash, TransactionVersion,
};
use tracing::{debug, trace, warn};

use super::{Error, MessagingConfig, Messenger, MessengerResult, LOG_TARGET};
use crate::backend::storage::transaction::L1HandlerTransaction;

abigen!(
    StarknetMessagingLocal,
    "contracts/messaging/solidity/IStarknetMessagingLocal_ABI.json",
    event_derives(serde::Serialize, serde::Deserialize)
);

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

pub struct EthereumMessaging {
    provider: Arc<Provider<Http>>,
    provider_signer: Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    messaging_contract_address: Address,
}

impl EthereumMessaging {
    pub async fn new(config: MessagingConfig) -> Result<EthereumMessaging> {
        let provider = Provider::<Http>::try_from(&config.rpc_url)?;

        let chain_id = provider.get_chainid().await?;

        let wallet: LocalWallet =
            config.private_key.parse::<LocalWallet>()?.with_chain_id(chain_id.as_u32());

        let provider_signer = SignerMiddleware::new(provider.clone(), wallet);
        let messaging_contract_address = Address::from_str(&config.contract_address)?;

        Ok(EthereumMessaging {
            provider: Arc::new(provider),
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
        trace!(target: LOG_TARGET, "Fetching logs for blocks {} - {}.", from_block, to_block);

        let mut logs: HashMap<u64, Vec<Log>> = HashMap::new();

        let log_msg_to_l2_topic =
            H256::from_str("0xdb80dd488acf86d17c747445b0eabb5d57c541d3bd7b6b87af987858e5066b2b")
                .unwrap();

        let filters = Filter {
            block_option: FilterBlockOption::Range {
                from_block: Some(BlockNumber::Number(from_block.into())),
                to_block: Some(BlockNumber::Number(to_block.into())),
            },
            address: Some(ValueOrArray::Value(self.messaging_contract_address)),
            topics: [Some(ValueOrArray::Value(Some(log_msg_to_l2_topic))), None, None, None],
        };

        self.provider
            .get_logs(&filters)
            .await?
            .iter()
            .filter(|&l| l.block_number.is_some())
            .for_each(|l| {
                logs.entry(
                    l.block_number
                           .unwrap() // safe as we filter on Some only.
                           .try_into()
                           .expect("Block number couldn't be converted to u64."),
                )
                .and_modify(|v| v.push(l.clone()))
                .or_insert(vec![l.clone()]);
            });

        Ok(logs)
    }
}

#[async_trait]
impl Messenger for EthereumMessaging {
    type MessageHash = U256;

    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
    ) -> MessengerResult<(u64, Vec<L1HandlerTransaction>)> {
        let chain_latest_block: u64 = self
            .provider
            .get_block_number()
            .await?
            .try_into()
            .expect("Can't convert latest block number into u64.");

        // +1 as the from_block counts as 1 block fetched.
        let to_block = if from_block + max_blocks + 1 < chain_latest_block {
            from_block + max_blocks
        } else {
            chain_latest_block
        };

        let mut l1_handler_txs = vec![];

        self.fetch_logs(from_block, to_block).await?.iter().for_each(
            |(block_number, block_logs)| {
                debug!(
                    target: LOG_TARGET,
                    "Converting logs of block {} into L1HandlerTx ({} logs)",
                    block_number,
                    block_logs.len(),
                );

                block_logs.iter().for_each(|l| {
                    if let Ok(tx) = l1_handler_tx_from_log(l) {
                        l1_handler_txs.push(tx)
                    }
                })
            },
        );

        Ok((to_block, l1_handler_txs))
    }

    async fn settle_messages(
        &self,
        messages: &[MsgToL1],
    ) -> MessengerResult<Vec<Self::MessageHash>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let starknet_messaging = StarknetMessagingLocal::new(
            self.messaging_contract_address,
            self.provider_signer.clone(),
        );

        let hashes = parse_messages(messages);

        debug!("Sending transaction on L1 to register messages...");
        match starknet_messaging
            .add_message_hashes_from_l2(hashes.clone())
            .send()
            .await
            .map_err(|_| Error::SendError)?
            // wait for the tx to be mined
            .await?
        {
            Some(receipt) => {
                trace!(
                    target: LOG_TARGET,
                    "Transaction sent on L1 to settle {} messages: {:#x}",
                    hashes.len(),
                    receipt.transaction_hash,
                );

                Ok(hashes)
            }
            None => {
                warn!(target: LOG_TARGET, "No receipt for L1 transaction.");
                Err(Error::SendError)
            }
        }
    }
}

/// Converts a starknet core log into a L1 handler transaction.
fn l1_handler_tx_from_log(log: &Log) -> Result<L1HandlerTransaction> {
    let parsed_log = <LogMessageToL2 as EthLogDecode>::decode_log(&log.clone().into())?;

    let from_address = stark_felt_from_address(parsed_log.from_address);
    let contract_address = stark_felt_from_u256(parsed_log.to_address);
    let selector = stark_felt_from_u256(parsed_log.selector);
    let nonce = stark_felt_from_u256(parsed_log.nonce);
    let fee: u128 = parsed_log.fee.try_into().expect("Fee does not fit into u128.");

    let mut calldata_vec = vec![from_address];
    for p in parsed_log.payload {
        calldata_vec.push(stark_felt_from_u256(p));
    }

    let calldata = Calldata(calldata_vec.into());

    // TODO: not sure about how this must be computed,
    // at least with a nonce + address we should be
    // ok for now? Or is it derived from something?
    let tx_hash = hash::pedersen_hash(&nonce, &contract_address);

    let tx = L1HandlerTransaction {
        inner: ApiL1HandlerTransaction {
            transaction_hash: TransactionHash(tx_hash),
            version: TransactionVersion(stark_felt!(1_u32)),
            nonce: Nonce(nonce),
            contract_address: ContractAddress::try_from(contract_address).unwrap(),
            entry_point_selector: EntryPointSelector(selector),
            calldata,
        },
        paid_l1_fee: fee,
    };

    Ok(tx)
}

/// With Ethereum, the messages are following the conventional starknet messaging.
/// There is no MSG/EXE MAGIC expected here.
fn parse_messages(messages: &[MsgToL1]) -> Vec<U256> {
    messages
        .iter()
        .map(|msg| {
            let mut buf: Vec<u8> = vec![];
            buf.extend(msg.from_address.to_bytes_be());
            buf.extend(msg.to_address.to_bytes_be());
            buf.extend(FieldElement::from(msg.payload.len()).to_bytes_be());
            msg.payload.iter().for_each(|p| buf.extend(p.to_bytes_be()));

            let mut hasher = Keccak256::new();
            hasher.update(buf);
            let hash = hasher.finalize();
            let hash_bytes = hash.as_slice();
            U256::from_big_endian(hash_bytes)
        })
        .collect()
}

fn stark_felt_from_u256(v: U256) -> StarkFelt {
    stark_felt!(format!("{:#064x}", v).as_str())
}

fn stark_felt_from_address(v: Address) -> StarkFelt {
    stark_felt!(format!("{:#064x}", v).as_str())
}

#[cfg(test)]
mod tests {

    use starknet::macros::selector;

    use super::*;

    #[test]
    fn parse_messages_msg() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let payload = vec![FieldElement::ONE, FieldElement::TWO];

        let messages = vec![MsgToL1 { from_address, to_address, payload }];

        let hashes = parse_messages(&messages);
        assert_eq!(hashes.len(), 1);
        assert_eq!(
            hashes[0],
            U256::from_str_radix(
                "0x5ba1d2e131360f15e26dd4f6ff10550685611cc25f75e7950b704adb04b36162",
                16
            )
            .unwrap()
        );
    }

    #[test]
    fn l1_handler_tx_from_event_parse_ok() {
        let from_address = "0x000000000000000000000000be3C44c09bc1a3566F3e1CA12e5AbA0fA4Ca72Be";
        let to_address = "0x039dc79e64f4bb3289240f88e0bae7d21735bef0d1a51b2bf3c4730cb16983e1";
        let selector = "0x02f15cff7b0eed8b9beb162696cf4e3e0e35fa7032af69cd1b7d2ac67a13f40f";
        let nonce = 783082_u128;
        let fee = 30000_u128;

        // Payload two values: [1, 2].
        let payload_buf = hex::decode("000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000bf2ea0000000000000000000000000000000000000000000000000000000000007530000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002").unwrap();

        let calldata: Vec<StarkFelt> = vec![
            FieldElement::from_hex_be(from_address).unwrap().into(),
            FieldElement::ONE.into(),
            FieldElement::TWO.into(),
        ];

        let transaction_hash: FieldElement = hash::pedersen_hash(
            &nonce.into(),
            &FieldElement::from_hex_be(to_address).unwrap().into(),
        )
        .into();

        let log = Log {
            address: H160::from_str("0xde29d060D45901Fb19ED6C6e959EB22d8626708e").unwrap(),
            topics: vec![
                H256::from_str(
                    "0xdb80dd488acf86d17c747445b0eabb5d57c541d3bd7b6b87af987858e5066b2b",
                )
                .unwrap(),
                H256::from_str(from_address).unwrap(),
                H256::from_str(to_address).unwrap(),
                H256::from_str(selector).unwrap(),
            ],
            data: payload_buf.into(),
            ..Default::default()
        };

        let expected = L1HandlerTransaction {
            inner: ApiL1HandlerTransaction {
                transaction_hash: TransactionHash(transaction_hash.into()),
                version: TransactionVersion(stark_felt!(1_u32)),
                nonce: Nonce(FieldElement::from(nonce).into()),
                contract_address: ContractAddress::try_from(
                    <FieldElement as Into<StarkFelt>>::into(
                        FieldElement::from_hex_be(to_address).unwrap(),
                    ),
                )
                .unwrap(),
                entry_point_selector: EntryPointSelector(
                    FieldElement::from_hex_be(selector).unwrap().into(),
                ),
                calldata: Calldata(calldata.into()),
            },
            paid_l1_fee: fee,
        };

        let tx = l1_handler_tx_from_log(&log).expect("aa");

        assert_eq!(tx.inner, expected.inner);
    }
}
