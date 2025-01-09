use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use katana_primitives::chain::ChainId;
use katana_primitives::receipt::MessageToL1;
use katana_primitives::transaction::L1HandlerTx;
use katana_primitives::utils::transaction::compute_l2_to_l1_message_hash;
use starknet::accounts::{Account, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, Call, EmittedEvent, EventFilter, Felt};
use starknet::macros::{felt, selector};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{AnyProvider, JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use tracing::{debug, error, trace, warn};
use url::Url;

use super::{Error, MessagingConfig, Messenger, MessengerResult, LOG_TARGET};

/// As messaging in starknet is only possible with EthAddress in the `to_address`
/// field, in teh current design we set the `to_address` to the `MSG` magic value.
///
/// Blockifier is the one responsible for this out of range error.
/// <https://github.com/starkware-libs/sequencer/blob/f4b25dd4689ba8ddec3c7db57ea7e8fd7ce32eab/crates/blockifier/src/execution/call_info.rs#L41>
const MSG_MAGIC: Felt = felt!("0x4d5347");

/// TODO: This may come from the configuration.
const MESSAGE_SENT_EVENT_KEY: Felt = selector!("MessageSent");

#[derive(Debug)]
pub struct StarknetMessaging {
    chain_id: Felt,
    provider: AnyProvider,
    wallet: LocalWallet,
    sender_account_address: Felt,
    messaging_contract_address: Felt,
}

impl StarknetMessaging {
    pub async fn new(config: MessagingConfig) -> Result<StarknetMessaging> {
        let provider = AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(
            Url::parse(&config.rpc_url)?,
        )));

        let private_key = Felt::from_hex(&config.private_key)?;
        let key = SigningKey::from_secret_scalar(private_key);
        let wallet = LocalWallet::from_signing_key(key);

        let chain_id = provider.chain_id().await?;
        let sender_account_address = Felt::from_hex(&config.sender_address)?;
        let messaging_contract_address = Felt::from_hex(&config.contract_address)?;

        Ok(StarknetMessaging {
            wallet,
            provider,
            chain_id,
            sender_account_address,
            messaging_contract_address,
        })
    }

    pub async fn fetch_events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
    ) -> Result<Vec<EmittedEvent>> {
        trace!(target: LOG_TARGET, from_block = ?from_block, to_block = ?to_block, "Fetching logs.");

        let mut events = vec![];

        let filter = EventFilter {
            from_block: Some(from_block),
            to_block: Some(to_block),
            address: Some(self.messaging_contract_address),
            keys: Some(vec![vec![MESSAGE_SENT_EVENT_KEY]]),
        };

        // TODO: This chunk_size may also come from configuration?
        let chunk_size = 200;
        let mut continuation_token: Option<String> = None;

        loop {
            let event_page =
                self.provider.get_events(filter.clone(), continuation_token, chunk_size).await?;

            event_page.events.into_iter().for_each(|event| {
                // We ignore events without the block number
                if event.block_number.is_some() {
                    // Blocks are processed in order as retrieved by `get_events`.
                    // This way we keep the order and ensure the messages are executed in order.
                    events.push(event);
                }
            });

            continuation_token = event_page.continuation_token;

            if continuation_token.is_none() {
                break;
            }
        }

        Ok(events)
    }

    /// Sends an invoke TX on starknet.
    async fn send_invoke_tx(&self, calls: Vec<Call>) -> Result<Felt> {
        let signer = Arc::new(&self.wallet);

        let mut account = SingleOwnerAccount::new(
            &self.provider,
            signer,
            self.sender_account_address,
            self.chain_id,
            ExecutionEncoding::New,
        );

        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        // TODO: we need to have maximum fee configurable.
        let execution = account.execute_v1(calls).fee_estimate_multiplier(10f64);
        let estimated_fee = (execution.estimate_fee().await?.overall_fee) * Felt::from(10u128);
        let tx = execution.max_fee(estimated_fee).send().await?;

        Ok(tx.transaction_hash)
    }

    /// Sends messages hashes to settlement layer by sending a transaction.
    async fn send_hashes(&self, hashes: Vec<Felt>) -> MessengerResult<Felt> {
        if hashes.is_empty() {
            return Ok(Felt::ZERO);
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
                trace!(target: LOG_TARGET, tx_hash = %format!("{:#064x}", tx_hash), "Hashes sending transaction.");
                Ok(tx_hash)
            }
            Err(e) => {
                error!(target: LOG_TARGET, error = %e, "Settling hashes on Starknet.");
                Err(Error::SendError)
            }
        }
    }
}

#[async_trait]
impl Messenger for StarknetMessaging {
    type MessageHash = Felt;
    type MessageTransaction = L1HandlerTx;

    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
        chain_id: ChainId,
    ) -> MessengerResult<(u64, Vec<Self::MessageTransaction>)> {
        let chain_latest_block: u64 = match self.provider.block_number().await {
            Ok(n) => n,
            Err(_) => {
                warn!(
                    target: LOG_TARGET,
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

        let mut l1_handler_txs: Vec<L1HandlerTx> = vec![];

        self.fetch_events(BlockId::Number(from_block), BlockId::Number(to_block))
            .await
            .map_err(|_| Error::SendError)
            .unwrap()
            .iter()
            .for_each(|e| {
                debug!(
                    target: LOG_TARGET,
                    event = ?e,
                    "Converting event into L1HandlerTx."
                );

                if let Ok(tx) = l1_handler_tx_from_event(e, chain_id) {
                    l1_handler_txs.push(tx)
                }
            });

        Ok((to_block, l1_handler_txs))
    }

    async fn send_messages(
        &self,
        messages: &[MessageToL1],
    ) -> MessengerResult<Vec<Self::MessageHash>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let hashes = parse_messages(messages)?;
        self.send_hashes(hashes.clone()).await?;

        Ok(hashes)
    }
}

/// Parses messages sent by cairo contracts to compute their hashes.
///
/// Messages can also be labelled as EXE, which in this case generate a `Call`
/// additionally to the hash.
fn parse_messages(messages: &[MessageToL1]) -> MessengerResult<Vec<Felt>> {
    let mut hashes: Vec<Felt> = vec![];

    for m in messages {
        // Field `to_address` is restricted to eth addresses space. So the
        // `to_address` is set to 'MSG' to indicate that the message
        // has to be sent to the L2 messaging contract.
        //
        // Blockifier is the one responsible for this out of range error.
        // <https://github.com/starkware-libs/sequencer/blob/f4b25dd4689ba8ddec3c7db57ea7e8fd7ce32eab/crates/blockifier/src/execution/call_info.rs#L41>
        let magic = m.to_address;

        if magic != MSG_MAGIC {
            warn!(target: LOG_TARGET, magic = %magic, "Skipping message with non-MSG magic.");
            continue;
        }

        // In the case or regular message, we compute the message's hash
        // which will then be sent in a transaction to be registered as being
        // ready for consumption by the L2 messaging contract.

        // As to_address is used by the magic, the `to_address` we want
        // is the first element of the payload.
        let to_address = m.payload[0];

        // Then, the payload must be changed to only keep the rest of the
        // data, without the first element that was the `to_address`.
        let payload = &m.payload[1..];

        let mut buf: Vec<Felt> = vec![m.from_address.into(), to_address, Felt::from(payload.len())];
        for p in payload {
            buf.push(*p);
        }

        // Piltover uses poseidon hash for all hashes computation.
        hashes.push(starknet_crypto::poseidon_hash_many(&buf));
    }

    Ok(hashes)
}

fn l1_handler_tx_from_event(event: &EmittedEvent, chain_id: ChainId) -> Result<L1HandlerTx> {
    if event.keys[0] != MESSAGE_SENT_EVENT_KEY {
        debug!(
            target: LOG_TARGET,
            event_key = ?event.keys[0],
            "Event can't be converted into L1HandlerTx."
        );
        return Err(Error::GatherError.into());
    }

    if event.keys.len() != 4 || event.data.len() < 2 {
        error!(target: LOG_TARGET, "Event MessageSentToAppchain is not well formatted.");
    }

    // See contrat appchain_messaging.cairo for MessageSentToAppchain event.
    let from_address = event.keys[2];
    let to_address = event.keys[3];
    let entry_point_selector = event.data[0];
    let nonce = event.data[1];

    // Skip the length of the serialized array for the payload which is data[2].
    // Payload starts at data[3].
    let mut calldata = vec![from_address];
    calldata.extend(&event.data[3..]);

    // TODO: this should be using the l1 -> l2 hash computation instead.
    // This needs to be adjusted to piltover, which actually uses poseidon.
    let message_hash = compute_l2_to_l1_message_hash(from_address, to_address, &calldata);

    Ok(L1HandlerTx {
        nonce,
        calldata,
        chain_id,
        message_hash,
        // This is the min value paid on L1 for the message to be sent to L2.
        paid_fee_on_l1: 30000_u128,
        entry_point_selector,
        version: Felt::ZERO,
        contract_address: to_address.into(),
    })
}

#[cfg(test)]
mod tests {

    use katana_primitives::utils::transaction::compute_l1_handler_tx_hash;
    use starknet::macros::felt;

    use super::*;

    #[test]
    fn parse_messages_msg() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let selector = selector!("selector");
        let payload_msg = vec![to_address, Felt::ONE, Felt::TWO];

        let messages = vec![MessageToL1 {
            from_address: from_address.into(),
            to_address: MSG_MAGIC,
            payload: payload_msg,
        }];

        let hashes = parse_messages(&messages).unwrap();

        assert_eq!(hashes.len(), 2);
        assert_eq!(
            hashes,
            vec![Felt::from_hex(
                "0x03a1d2e131360f15e26dd4f6ff10550685611cc25f75e7950b704adb04b36162"
            )
            .unwrap(),]
        );
    }

    #[test]
    #[should_panic]
    fn parse_messages_msg_bad_payload() {
        let from_address = selector!("from_address");
        let payload_msg = vec![];

        let messages = vec![MessageToL1 {
            from_address: from_address.into(),
            to_address: MSG_MAGIC,
            payload: payload_msg,
        }];

        parse_messages(&messages).unwrap();
    }

    #[test]
    fn l1_handler_tx_from_event_parse_ok() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let selector = selector!("selector");
        let chain_id = ChainId::parse("KATANA").unwrap();
        let nonce = Felt::ONE;
        let calldata = vec![from_address, Felt::THREE];

        let transaction_hash: Felt = compute_l1_handler_tx_hash(
            Felt::ZERO,
            to_address,
            selector,
            &calldata,
            chain_id.into(),
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
            data: vec![selector, nonce, Felt::from(calldata.len() as u128), Felt::THREE],
            block_hash: Some(selector!("block_hash")),
            block_number: Some(0),
            transaction_hash,
        };

        let message_hash = compute_l2_to_l1_message_hash(from_address, to_address, &calldata);

        let expected = L1HandlerTx {
            nonce,
            calldata,
            chain_id,
            message_hash,
            paid_fee_on_l1: 30000_u128,
            version: Felt::ZERO,
            entry_point_selector: selector,
            contract_address: to_address.into(),
        };

        let tx = l1_handler_tx_from_event(&event, chain_id).unwrap();

        assert_eq!(tx, expected);
    }

    #[test]
    #[should_panic]
    fn l1_handler_tx_from_event_parse_bad_selector() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let selector = selector!("selector");
        let nonce = Felt::ONE;
        let calldata = [from_address, Felt::THREE];
        let transaction_hash = Felt::ZERO;

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
            data: vec![selector, nonce, Felt::from(calldata.len() as u128), Felt::THREE],
            block_hash: Some(selector!("block_hash")),
            block_number: Some(0),
            transaction_hash,
        };

        let _tx = l1_handler_tx_from_event(&event, ChainId::default()).unwrap();
    }

    #[test]
    #[should_panic]
    fn l1_handler_tx_from_event_parse_missing_key_data() {
        let from_address = selector!("from_address");
        let _to_address = selector!("to_address");
        let _selector = selector!("selector");
        let _nonce = Felt::ONE;
        let _calldata = [from_address, Felt::THREE];
        let transaction_hash = Felt::ZERO;

        let event = EmittedEvent {
            from_address: felt!(
                "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"
            ),
            keys: vec![selector!("AnOtherUnexpectedEvent"), selector!("random_hash"), from_address],
            data: vec![],
            block_hash: Some(selector!("block_hash")),
            block_number: Some(0),
            transaction_hash,
        };

        let _tx = l1_handler_tx_from_event(&event, ChainId::default()).unwrap();
    }
}
