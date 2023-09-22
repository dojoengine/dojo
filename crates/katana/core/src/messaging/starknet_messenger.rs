use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use starknet::accounts::{Account, Call, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, EmittedEvent, EventFilter, FieldElement, MsgToL1};
use starknet::core::utils::starknet_keccak;
use starknet::macros::{felt, selector};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{AnyProvider, JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use starknet_api::core::{ContractAddress, EntryPointSelector, Nonce};
use starknet_api::hash::{self, StarkFelt};
use starknet_api::stark_felt;
use starknet_api::transaction::{
    Calldata, L1HandlerTransaction as ApiL1HandlerTransaction, TransactionHash, TransactionVersion,
};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, error, trace, warn};
use url::Url;

use crate::backend::storage::transaction::L1HandlerTransaction;
use crate::messaging::{
    MessagingConfig, Messenger, MessengerError, MessengerResult, MSGING_TARGET,
};

/// As messaging in starknet is only possible with EthAddress in the `to_address`
/// field, we have to set magic value to understand what the user want to do.
/// In the case of execution -> the felt 'EXE' will be passed.
/// And for normal messages, the felt 'MSG' is used.
/// Those values are very not likely a valid account address on starknet.
const MSG_MAGIC: FieldElement = felt!("0x4d5347");
const EXE_MAGIC: FieldElement = felt!("0x455845");

pub const HASH_EXEC: FieldElement = felt!("0xee");

///
pub struct StarknetMessenger {
    chain_id: FieldElement,
    provider: AnyProvider,
    wallet: LocalWallet,
    sender_account_address: FieldElement,
    messaging_contract_address: FieldElement,
}

impl StarknetMessenger {
    pub async fn new(config: MessagingConfig) -> Result<Arc<AsyncRwLock<StarknetMessenger>>> {
        let provider = AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(
            Url::parse(&config.rpc_url)?,
        )));

        let private_key = FieldElement::from_hex_be(&config.private_key)?;
        let key = SigningKey::from_secret_scalar(private_key);
        let wallet = LocalWallet::from_signing_key(key);

        let chain_id = provider.chain_id().await?;

        let sender_account_address = FieldElement::from_hex_be(&config.sender_address)?;

        let messaging_contract_address = FieldElement::from_hex_be(&config.contract_address)?;

        Ok(Arc::new(AsyncRwLock::new(StarknetMessenger {
            chain_id,
            provider,
            wallet,
            sender_account_address,
            messaging_contract_address,
        })))
    }

    /// Fetches events for the given blocks range.
    pub async fn fetch_events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
    ) -> Result<HashMap<u64, Vec<EmittedEvent>>> {
        trace!(
            target: MSGING_TARGET,
            "Fetching blocks {:?} - {:?}.", from_block, to_block);

        let mut events: HashMap<u64, Vec<EmittedEvent>> = HashMap::new();

        let filter = EventFilter {
            from_block: Some(from_block),
            to_block: Some(to_block),
            address: Some(self.messaging_contract_address),
            // TODO: this might come from the configuration actually.
            keys: None,
        };

        // TODO: this chunk_size may also come from configuration?
        let chunk_size = 200;
        let mut continuation_token: Option<String> = None;

        loop {
            let event_page =
                self.provider.get_events(filter.clone(), continuation_token, chunk_size).await?;

            event_page.events.iter().for_each(|e| {
                events
                    .entry(e.block_number)
                    .and_modify(|v| v.push(e.clone()))
                    .or_insert(vec![e.clone()]);
            });

            continuation_token = event_page.continuation_token;

            if continuation_token.is_none() {
                break;
            }
        }

        Ok(events)
    }

    /// Sends an invoke TX on starknet.
    async fn send_invoke_tx(&self, calls: Vec<Call>) -> Result<FieldElement> {
        let signer = Arc::new(&self.wallet);

        let mut account = SingleOwnerAccount::new(
            &self.provider,
            signer,
            self.sender_account_address,
            self.chain_id,
            ExecutionEncoding::Legacy,
        );

        account.set_block_id(BlockId::Tag(BlockTag::Latest));

        // TODO: we need to have maximum fee configurable.
        let execution = account.execute(calls).fee_estimate_multiplier(10f64);
        let estimated_fee = (execution.estimate_fee().await?.overall_fee) * 10;
        let tx = execution.max_fee(estimated_fee.into()).send().await?;

        Ok(tx.transaction_hash)
    }

    /// Settles messages hashes by sending a transaction to the settlement
    /// layer.
    async fn settle_hashes(&self, hashes: &[FieldElement]) -> MessengerResult<FieldElement> {
        let mut hashes = hashes.to_vec();
        hashes.retain(|&x| x != HASH_EXEC);

        if hashes.is_empty() {
            return Ok(FieldElement::ZERO);
        }

        let selector = selector!("add_messages_hashes_from_appchain");

        let mut calldata = vec![FieldElement::from(hashes.len() as u128)];
        for h in hashes {
            calldata.push(h);
        }

        let calls = vec![Call { to: self.messaging_contract_address, selector, calldata }];

        match self.send_invoke_tx(calls).await {
            Ok(tx_hash) => {
                trace!(target: MSGING_TARGET,
                       "Settlement hashes transaction {:#064x}", tx_hash);
                Ok(tx_hash)
            }
            Err(e) => {
                error!("Error settling hashes on Starknet: {:?}", e);
                Err(MessengerError::SendError)
            }
        }
    }
}

#[async_trait]
impl Messenger for StarknetMessenger {
    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
    ) -> MessengerResult<(u64, Vec<L1HandlerTransaction>)> {
        let chain_latest_block: u64 = match self.provider.block_number().await {
            Ok(n) => n,
            Err(_) => {
                warn!(
                    "Couldn't fetch settlement chain last block number. \nSkipped, retry at the \
                     next tick."
                );
                return Err(MessengerError::SendError);
            }
        };

        if from_block > chain_latest_block {
            // Nothing to fetch, we can skip waiting the next tick.
            return Ok((chain_latest_block, vec![]));
        }

        // +1 as the from_block counts as 1 block fetched.
        let to_block = if from_block + max_blocks + 1 < chain_latest_block {
            from_block + max_blocks
        } else {
            chain_latest_block
        };

        let mut l1_handler_txs: Vec<L1HandlerTransaction> = vec![];

        self.fetch_events(BlockId::Number(from_block), BlockId::Number(to_block))
            .await
            .map_err(|_| MessengerError::SendError)
            .unwrap()
            .iter()
            .for_each(|(block_number, block_events)| {
                debug!(
                    target: MSGING_TARGET,
                    "Converting events of block {} into L1HandlerTx ({} events)",
                    block_number,
                    block_events.len(),
                );

                block_events.iter().for_each(|e| {
                    if let Ok(tx) = l1_handler_tx_from_event(e) {
                        l1_handler_txs.push(tx)
                    }
                })
            });

        Ok((to_block, l1_handler_txs))
    }

    async fn settle_messages(&self, messages: &[MsgToL1]) -> MessengerResult<Vec<String>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let mut hashes: Vec<FieldElement> = vec![];
        let mut calls: Vec<Call> = vec![];

        for m in messages {
            // Field `to_address` is restricted to eth addresses space. So the
            // `to_address` is set to 'EXE'/'MSG' to indicate that the message
            // has to be executed or sent normally.
            let magic = m.to_address;

            if magic == EXE_MAGIC {
                if m.payload.len() < 2 {
                    error!(
                        target: MSGING_TARGET,
                        "Message execution is expecting a payload of at least length \
                         2. With [0] being the contract address, and [1] the selector.",
                    );
                }

                let to = m.payload[0];
                let selector = m.payload[1];

                let mut calldata = vec![];
                // We must exclude the `to_address` and `selector` from the actual payload.
                if m.payload.len() >= 3 {
                    calldata.extend(m.payload[2..].to_vec());
                }

                calls.push(Call { to, selector, calldata });
                hashes.push(HASH_EXEC);
            } else if magic == MSG_MAGIC {
                // In the case or regular message, we compute the message's hash
                // which will then be sent in a transaction to be registered.

                // As to_address is used by the magic, the `to_address` we want
                // is the first element of the payload.
                let to_address = m.payload[0];

                // Then, the payload must be changed to only keep the rest of the
                // data, without the first element that was the `to_address`.
                let payload = &m.payload[1..];

                let mut buf: Vec<u8> = vec![];
                buf.extend(m.from_address.to_bytes_be());
                buf.extend(to_address.to_bytes_be());
                buf.extend(FieldElement::from(payload.len()).to_bytes_be());
                for p in payload {
                    buf.extend(p.to_bytes_be());
                }

                hashes.push(starknet_keccak(&buf));
            } else {
                // Skip the message if no valid magic number found.
                warn!("Invalid message to_address magic value: {:?}", magic);
                continue;
            }
        }

        if !calls.is_empty() {
            match self.send_invoke_tx(calls).await {
                Ok(tx_hash) => {
                    trace!(target: MSGING_TARGET,
                           "Invoke transaction hash {:#064x}", tx_hash);
                }
                Err(e) => {
                    error!("Error sending invoke tx on Starknet: {:?}", e);
                    return Err(MessengerError::SendError);
                }
            };
        }

        self.settle_hashes(&hashes).await?;

        Ok(hashes.iter().map(|h| format!("{:#064x}", h)).collect())
    }
}

fn l1_handler_tx_from_event(event: &EmittedEvent) -> Result<L1HandlerTransaction> {
    // TODO: replace by the keys directly in the configuration.
    if event.keys[0] != selector!("MessageSentToAppchain") {
        debug!(
            target: MSGING_TARGET,
            "Event with key {:?} can't be converted into L1HandlerTransaction",
            event.keys[0],
        );
        return Err(MessengerError::GatherError.into());
    }

    if event.keys.len() != 4 || event.data.len() < 2 {
        error!(
            target: MSGING_TARGET,
            "Event MessageSentToAppchain is not well formatted"
        );
    }

    // See contrat appchain_messaging.cairo for MessageSentToAppchain event.
    let from_address = event.keys[2];
    let to_address = event.keys[3];
    let selector = event.data[0];
    let nonce = event.data[1];

    // Skip the length of the serialized array for the payload which is data[2].
    // Payload starts at data[3].
    let mut calldata_vec: Vec<StarkFelt> = vec![from_address.into()];
    for p in &event.data[3..] {
        calldata_vec.push((*p).into());
    }

    let calldata = Calldata(calldata_vec.into());

    let tx_hash = hash::pedersen_hash(&nonce.into(), &to_address.into());

    let tx = L1HandlerTransaction {
        inner: ApiL1HandlerTransaction {
            transaction_hash: TransactionHash(tx_hash),
            version: TransactionVersion(stark_felt!(1_u32)),
            nonce: Nonce(nonce.into()),
            contract_address: ContractAddress::try_from(<FieldElement as Into<StarkFelt>>::into(
                to_address,
            ))
            .unwrap(),
            entry_point_selector: EntryPointSelector(selector.into()),
            calldata,
        },
        // TODO: fee is missing in the event, is this default value ok as it's the minimum one
        // expected on L1 usually, or should we put max value here?
        paid_l1_fee: 30000_u128,
    };

    Ok(tx)
}
