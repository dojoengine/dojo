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
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;
use starknet_api::transaction::{
    Calldata, L1HandlerTransaction as ApiL1HandlerTransaction, TransactionHash, TransactionVersion,
};
use tracing::{debug, error, trace, warn};
use url::Url;

use super::{Error, MessagingConfig, Messenger, MessengerResult, LOG_TARGET};
use crate::backend::storage::transaction::L1HandlerTransaction;
use crate::utils::transaction::compute_l1_handler_transaction_hash_felts;

/// As messaging in starknet is only possible with EthAddress in the `to_address`
/// field, we have to set magic value to understand what the user want to do.
/// In the case of execution -> the felt 'EXE' will be passed.
/// And for normal messages, the felt 'MSG' is used.
/// Those values are very not likely a valid account address on starknet.
const MSG_MAGIC: FieldElement = felt!("0x4d5347");
const EXE_MAGIC: FieldElement = felt!("0x455845");

pub const HASH_EXEC: FieldElement = felt!("0xee");

pub struct StarknetMessaging {
    chain_id: FieldElement,
    provider: AnyProvider,
    wallet: LocalWallet,
    sender_account_address: FieldElement,
    messaging_contract_address: FieldElement,
}

impl StarknetMessaging {
    pub async fn new(config: MessagingConfig) -> Result<StarknetMessaging> {
        let provider = AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(
            Url::parse(&config.rpc_url)?,
        )));

        let private_key = FieldElement::from_hex_be(&config.private_key)?;
        let key = SigningKey::from_secret_scalar(private_key);
        let wallet = LocalWallet::from_signing_key(key);

        let chain_id = provider.chain_id().await?;
        let sender_account_address = FieldElement::from_hex_be(&config.sender_address)?;
        let messaging_contract_address = FieldElement::from_hex_be(&config.contract_address)?;

        Ok(StarknetMessaging {
            wallet,
            provider,
            chain_id,
            sender_account_address,
            messaging_contract_address,
        })
    }

    /// Fetches events for the given blocks range.
    pub async fn fetch_events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
    ) -> Result<HashMap<u64, Vec<EmittedEvent>>> {
        trace!(target: LOG_TARGET, "Fetching blocks {:?} - {:?}.", from_block, to_block);

        let mut block_to_events: HashMap<u64, Vec<EmittedEvent>> = HashMap::new();

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

            event_page.events.into_iter().for_each(|event| {
                block_to_events
                    .entry(event.block_number)
                    .and_modify(|v| v.push(event.clone()))
                    .or_insert(vec![event]);
            });

            continuation_token = event_page.continuation_token;

            if continuation_token.is_none() {
                break;
            }
        }

        Ok(block_to_events)
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

    /// Sends messages hashes to settlement layer by sending a transaction.
    async fn send_hashes(&self, mut hashes: Vec<FieldElement>) -> MessengerResult<FieldElement> {
        hashes.retain(|&x| x != HASH_EXEC);

        if hashes.is_empty() {
            return Ok(FieldElement::ZERO);
        }

        let mut calldata = hashes;
        calldata.insert(0, calldata.len().into());

        let call = Call {
            selector: selector!("add_messages_hashes_from_appchain"),
            to: self.messaging_contract_address,
            calldata,
        };

        match self.send_invoke_tx(vec![call]).await {
            Ok(tx_hash) => {
                trace!(target: LOG_TARGET, "Hashes sending transaction {:#064x}", tx_hash);
                Ok(tx_hash)
            }
            Err(e) => {
                error!("Error settling hashes on Starknet: {:?}", e);
                Err(Error::SendError)
            }
        }
    }
}

#[async_trait]
impl Messenger for StarknetMessaging {
    type MessageHash = FieldElement;
    type MessageTransaction = L1HandlerTransaction;

    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
        chain_id: FieldElement,
    ) -> MessengerResult<(u64, Vec<Self::MessageTransaction>)> {
        let chain_latest_block: u64 = match self.provider.block_number().await {
            Ok(n) => n,
            Err(_) => {
                warn!(
                    "Couldn't fetch settlement chain last block number. \nSkipped, retry at the \
                     next tick."
                );
                return Err(Error::SendError);
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
            .map_err(|_| Error::SendError)
            .unwrap()
            .iter()
            .for_each(|(block_number, block_events)| {
                debug!(
                    target: LOG_TARGET,
                    "Converting events of block {} into L1HandlerTx ({} events)",
                    block_number,
                    block_events.len(),
                );

                block_events.iter().for_each(|e| {
                    if let Ok(tx) = l1_handler_tx_from_event(e, chain_id) {
                        l1_handler_txs.push(tx)
                    }
                })
            });

        Ok((to_block, l1_handler_txs))
    }

    async fn send_messages(&self, messages: &[MsgToL1]) -> MessengerResult<Vec<Self::MessageHash>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let (hashes, calls) = parse_messages(messages)?;

        if !calls.is_empty() {
            match self.send_invoke_tx(calls).await {
                Ok(tx_hash) => {
                    trace!(target: LOG_TARGET, "Invoke transaction hash {:#064x}", tx_hash);
                }
                Err(e) => {
                    error!("Error sending invoke tx on Starknet: {:?}", e);
                    return Err(Error::SendError);
                }
            };
        }

        self.send_hashes(hashes.clone()).await?;

        Ok(hashes)
    }
}

/// Parses messages sent by cairo contracts to compute their hashes.
///
/// Messages can also be labelled as EXE, which in this case generate a `Call`
/// additionally to the hash.
fn parse_messages(messages: &[MsgToL1]) -> MessengerResult<(Vec<FieldElement>, Vec<Call>)> {
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
                    target: LOG_TARGET,
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

    Ok((hashes, calls))
}

fn l1_handler_tx_from_event(
    event: &EmittedEvent,
    chain_id: FieldElement,
) -> Result<L1HandlerTransaction> {
    if event.keys[0] != selector!("MessageSentToAppchain") {
        debug!(
            target: LOG_TARGET,
            "Event with key {:?} can't be converted into L1HandlerTransaction", event.keys[0],
        );
        return Err(Error::GatherError.into());
    }

    if event.keys.len() != 4 || event.data.len() < 2 {
        error!(target: LOG_TARGET, "Event MessageSentToAppchain is not well formatted");
    }

    // See contrat appchain_messaging.cairo for MessageSentToAppchain event.
    let from_address = event.keys[2];
    let to_address = event.keys[3];
    let selector = event.data[0];
    let nonce = event.data[1];
    let version = 0_u32;

    // Skip the length of the serialized array for the payload which is data[2].
    // Payload starts at data[3].
    let mut calldata = vec![from_address];
    calldata.extend(&event.data[3..]);

    let tx_hash = compute_l1_handler_transaction_hash_felts(
        version.into(),
        to_address,
        selector,
        &calldata,
        chain_id,
        nonce,
    );

    let calldata: Vec<StarkFelt> = calldata.iter().map(|f| StarkFelt::from(*f)).collect();
    let calldata = Calldata(calldata.into());

    let tx = L1HandlerTransaction {
        inner: ApiL1HandlerTransaction {
            transaction_hash: TransactionHash(tx_hash.into()),
            version: TransactionVersion(stark_felt!(version)),
            nonce: Nonce(nonce.into()),
            contract_address: ContractAddress::try_from(<FieldElement as Into<StarkFelt>>::into(
                to_address,
            ))
            .unwrap(),
            entry_point_selector: EntryPointSelector(selector.into()),
            calldata,
        },
        // This is the min value paid on L1 for the message to be sent to L2.
        paid_l1_fee: 30000_u128,
    };

    Ok(tx)
}

#[cfg(test)]
mod tests {

    use starknet::macros::felt;

    use super::*;
    use crate::utils::transaction::stark_felt_to_field_element_array;

    #[test]
    fn parse_messages_msg() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let selector = selector!("selector");
        let payload_msg = vec![to_address, FieldElement::ONE, FieldElement::TWO];
        let payload_exe = vec![to_address, selector, FieldElement::ONE, FieldElement::TWO];

        let messages = vec![
            MsgToL1 { from_address, to_address: MSG_MAGIC, payload: payload_msg },
            MsgToL1 { from_address, to_address: EXE_MAGIC, payload: payload_exe.clone() },
        ];

        let (hashes, calls) = parse_messages(&messages).unwrap();

        assert_eq!(hashes.len(), 2);
        assert_eq!(
            hashes,
            vec![
                FieldElement::from_hex_be(
                    "0x03a1d2e131360f15e26dd4f6ff10550685611cc25f75e7950b704adb04b36162"
                )
                .unwrap(),
                HASH_EXEC,
            ]
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, to_address);
        assert_eq!(calls[0].selector, selector);
        assert_eq!(calls[0].calldata, payload_exe[2..].to_vec());
    }

    #[test]
    #[should_panic]
    fn parse_messages_msg_bad_payload() {
        let from_address = selector!("from_address");
        let payload_msg = vec![];

        let messages = vec![MsgToL1 { from_address, to_address: MSG_MAGIC, payload: payload_msg }];

        parse_messages(&messages).unwrap();
    }

    #[test]
    #[should_panic]
    fn parse_messages_exe_bad_payload() {
        let from_address = selector!("from_address");
        let payload_exe = vec![FieldElement::ONE];

        let messages = vec![MsgToL1 { from_address, to_address: EXE_MAGIC, payload: payload_exe }];

        parse_messages(&messages).unwrap();
    }

    #[test]
    fn l1_handler_tx_from_event_parse_ok() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let selector = selector!("selector");
        let chain_id = selector!("KATANA");
        let nonce = FieldElement::ONE;
        let calldata: Vec<StarkFelt> = vec![from_address.into(), FieldElement::THREE.into()];
        let transaction_hash: FieldElement = compute_l1_handler_transaction_hash_felts(
            FieldElement::ZERO,
            to_address,
            selector,
            &stark_felt_to_field_element_array(&calldata),
            chain_id,
            nonce,
        );

        let event = EmittedEvent {
            from_address: felt!(
                "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"
            ),
            keys: vec![
                selector!("MessageSentToAppchain"),
                selector!("random_hash"),
                from_address,
                to_address,
            ],
            data: vec![
                selector,
                nonce,
                FieldElement::from(calldata.len() as u128),
                FieldElement::THREE,
            ],
            block_hash: selector!("block_hash"),
            block_number: 0,
            transaction_hash,
        };

        let expected = L1HandlerTransaction {
            inner: ApiL1HandlerTransaction {
                transaction_hash: TransactionHash(transaction_hash.into()),
                version: TransactionVersion(stark_felt!(0_u32)),
                nonce: Nonce(nonce.into()),
                contract_address: ContractAddress::try_from(
                    <FieldElement as Into<StarkFelt>>::into(to_address),
                )
                .unwrap(),
                entry_point_selector: EntryPointSelector(selector.into()),
                calldata: Calldata(calldata.into()),
            },
            paid_l1_fee: 30000_u128,
        };

        let tx = l1_handler_tx_from_event(&event, chain_id).unwrap();

        assert_eq!(tx.inner, expected.inner);
    }

    #[test]
    #[should_panic]
    fn l1_handler_tx_from_event_parse_bad_selector() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let selector = selector!("selector");
        let nonce = FieldElement::ONE;
        let calldata: Vec<StarkFelt> = vec![from_address.into(), FieldElement::THREE.into()];
        let transaction_hash = FieldElement::ZERO;

        let event = EmittedEvent {
            from_address: felt!(
                "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"
            ),
            keys: vec![
                selector!("AnOtherUnexpectedEvent"),
                selector!("random_hash"),
                from_address,
                to_address,
            ],
            data: vec![
                selector,
                nonce,
                FieldElement::from(calldata.len() as u128),
                FieldElement::THREE,
            ],
            block_hash: selector!("block_hash"),
            block_number: 0,
            transaction_hash,
        };

        let _tx = l1_handler_tx_from_event(&event, FieldElement::ZERO).unwrap();
    }

    #[test]
    #[should_panic]
    fn l1_handler_tx_from_event_parse_missing_key_data() {
        let from_address = selector!("from_address");
        let _to_address = selector!("to_address");
        let _selector = selector!("selector");
        let _nonce = FieldElement::ONE;
        let _calldata: Vec<StarkFelt> = vec![from_address.into(), FieldElement::THREE.into()];
        let transaction_hash = FieldElement::ZERO;

        let event = EmittedEvent {
            from_address: felt!(
                "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"
            ),
            keys: vec![selector!("AnOtherUnexpectedEvent"), selector!("random_hash"), from_address],
            data: vec![],
            block_hash: selector!("block_hash"),
            block_number: 0,
            transaction_hash,
        };

        let _tx = l1_handler_tx_from_event(&event, FieldElement::ZERO).unwrap();
    }
}
