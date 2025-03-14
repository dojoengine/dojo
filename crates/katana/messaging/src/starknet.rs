use alloy_primitives::B256;
use anyhow::Result;
use async_trait::async_trait;
use katana_primitives::chain::ChainId;
use katana_primitives::transaction::L1HandlerTx;
use starknet::core::types::{BlockId, EmittedEvent, EventFilter, Felt};
use starknet::macros::selector;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{AnyProvider, JsonRpcClient, Provider};
use tracing::{debug, error, trace, warn};
use url::Url;

use super::{Error, MessagingConfig, Messenger, MessengerResult, LOG_TARGET};

/// TODO: This may come from the configuration.
pub const MESSAGE_SENT_EVENT_KEY: Felt = selector!("MessageSent");

#[derive(Debug)]
pub struct StarknetMessaging {
    provider: AnyProvider,
    messaging_contract_address: Felt,
}

impl StarknetMessaging {
    pub async fn new(config: MessagingConfig) -> Result<StarknetMessaging> {
        let provider = AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(
            Url::parse(&config.rpc_url)?,
        )));

        let messaging_contract_address = Felt::from_hex(&config.contract_address)?;

        Ok(StarknetMessaging { provider, messaging_contract_address })
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
                return Err(Error::GatherError);
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
            .map_err(|_| Error::GatherError)
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
}

fn l1_handler_tx_from_event(event: &EmittedEvent, chain_id: ChainId) -> Result<L1HandlerTx> {
    if event.keys[0] != MESSAGE_SENT_EVENT_KEY {
        error!(
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

    let message_hash = compute_starknet_to_appchain_message_hash(
        from_address,
        to_address,
        nonce,
        entry_point_selector,
        &calldata,
    );

    let message_hash = B256::from_slice(message_hash.to_bytes_be().as_slice());

    Ok(L1HandlerTx {
        nonce,
        calldata,
        chain_id,
        message_hash,
        // This is the min value paid on L1 for the message to be sent to L2.
        // This doesn't apply for l2-l3 messaging in the current setting.
        paid_fee_on_l1: 30000_u128,
        entry_point_selector,
        version: Felt::ZERO,
        contract_address: to_address.into(),
    })
}

/// Computes the hash of a L2 to L3 message.
///
/// Piltover uses poseidon hash for all hashes computation.
/// <https://github.com/keep-starknet-strange/piltover/blob/a9c015eada5082076185a7b1413163a3da247009/src/messaging/hash.cairo#L22>
fn compute_starknet_to_appchain_message_hash(
    from_address: Felt,
    to_address: Felt,
    nonce: Felt,
    entry_point_selector: Felt,
    payload: &[Felt],
) -> Felt {
    let mut buf: Vec<Felt> =
        vec![from_address, to_address, nonce, entry_point_selector, Felt::from(payload.len())];
    for p in payload {
        buf.push(*p);
    }

    starknet_crypto::poseidon_hash_many(&buf)
}

#[cfg(test)]
mod tests {
    use katana_primitives::utils::transaction::compute_l1_handler_tx_hash;
    use starknet::macros::felt;

    use super::*;

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
            keys: vec![MESSAGE_SENT_EVENT_KEY, selector!("random_hash"), from_address, to_address],
            data: vec![selector, nonce, Felt::from(calldata.len() as u128), Felt::THREE],
            block_hash: Some(selector!("block_hash")),
            block_number: Some(0),
            transaction_hash,
        };

        let message_hash = compute_starknet_to_appchain_message_hash(
            from_address,
            to_address,
            nonce,
            selector,
            &calldata,
        );

        let expected = L1HandlerTx {
            nonce,
            calldata,
            chain_id,
            message_hash: B256::from_slice(message_hash.to_bytes_be().as_slice()),
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
