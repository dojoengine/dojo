mod ethereum_messenger;
mod starknet_messenger;
mod any_messenger;

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::time;
use starknet::core::types::{MsgToL1, FieldElement};
use ethers::providers::ProviderError;

use crate::sequencer::SequencerMessagingConfig;
use crate::backend::{Backend, storage::transaction::{Transaction, L1HandlerTransaction}}
;
use any_messenger::AnyMessenger;

type MessengerResult<T> = Result<T, MessengerError>;

#[derive(Debug, thiserror::Error)]
pub enum MessengerError {
    #[error("Error initializing messaging, please check messaging args")]
    InitError,
    #[error("Error gathering messages")]
    GatherError,
    #[error("Error sending messages")]
    SendError,
    #[error("Error ethereum provider: {0}")]
    EthereumProviderError(ProviderError),
}

#[async_trait]
pub trait Messenger {
    /// Gathers messages emitted on the settlement chain and returns the
    /// list of transaction (L1HanlderTx) to be executed and the last fetched block.
    ///
    /// # Arguments
    ///
    /// * `from_block` - From which block the messages should be gathered.
    async fn gather_messages(&self, from_block: u64, max_blocks: u64) -> MessengerResult<(u64, Vec<L1HandlerTransaction>)>;

    /// Computes the hash of the given messages and sends them to the settlement chain.
    ///
    /// Once message's hash is settled, one must send a transaction (with the message content)
    /// on the settlement chain to actually consume it.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to settle.
    async fn settle_messages(&self, messages: &Vec<MsgToL1>) -> MessengerResult<Vec<String>>;
}

///
pub async fn messaging_main_loop(
    config: SequencerMessagingConfig,
    starknet: Arc<Backend>
) -> MessengerResult<()> {

    let messenger: AnyMessenger = any_messenger::from_config(config.clone()).await?;

    match messenger {
        AnyMessenger::Ethereum(_) => tracing::debug!("Messaging enabled [Ethereum]"),
        AnyMessenger::Starknet(_) => tracing::debug!("Messaging enabled [Starknet]"),
    };

    let worker: Arc<Worker> = Arc::new(Worker {
        starknet,
        messenger: Arc::new(messenger),
    });

    tracing::debug!("Messaging enabled {:?}", config);

    // TODO: check how this can be easier to configure.
    let max_blocks = 200;

    let mut local_latest_block_number: u64 = 0;
    let mut settlement_latest_block_number: u64 = 0;

    loop {
        time::sleep(time::Duration::from_secs(config.fetch_interval)).await;

        (local_latest_block_number, _)
            = worker.settle_messages(local_latest_block_number).await?;

        (settlement_latest_block_number, _)
            = worker.gather_messages(settlement_latest_block_number, max_blocks).await;
    }
}

struct Worker {
    starknet: Arc<Backend>,
    messenger: Arc<AnyMessenger>,
}

impl Worker {
    /// Parses the local blocks transactions to find messages ready to be sent.
    /// Returns the latest processed block, and the count of messages sent.
    async fn settle_messages(&self, from_block: u64) -> MessengerResult<(u64, u64)> {
        let local_latest = self.starknet.storage.read().await.latest_number;
        tracing::debug!("Latest local block: {}", local_latest);

        if from_block > local_latest {
            return Ok((from_block, 0));
        }

        let mut n_sent = 0;

        for i in from_block..=local_latest {
            if let Some(block) = self.starknet.storage.read().await.block_by_number(i) {
                for o in &block.outputs {
                    match self.messenger.settle_messages(&o.messages_sent).await {
                        Ok(hashes) => {
                            trace_msg_to_l1_sent(&o.messages_sent, &hashes);
                            n_sent += o.messages_sent.len() as u64;
                        },
                        Err(e) => tracing::warn!("Error settling messages for block {}: {:?}", i, e),
                    };
                }   
            }
        }

        // +1 to ensure last block is not checked before the latest changes.
        Ok((local_latest + 1, n_sent))
    }

    /// Fetches messages from the settlement chain.
    /// Returns the latest fetched block, and the count of messages gathered.
    async fn gather_messages(&self, from_block: u64, max_blocks: u64) -> (u64, u64) {
        if let Ok((latest_block_fetched, l1_handler_txs))
            = self.messenger.gather_messages(from_block, max_blocks).await
        {
            for tx in &l1_handler_txs {
                trace_l1_handler_tx_exec(&tx);
                self.starknet.handle_transaction(Transaction::L1Handler(tx.clone())).await;
            }
            
            return (latest_block_fetched + 1, l1_handler_txs.len() as u64)
        } else {
            (from_block, 0)
        }
    }
}

fn trace_msg_to_l1_sent(messages: &Vec<MsgToL1>, hashes: &Vec<String>) {
    assert_eq!(messages.len(), hashes.len());

    for (i, m) in messages.iter().enumerate() {
        let payload_str: Vec<String> = m.payload
            .iter()
            .map(|f| format!("{:#x}", *f)).collect();

        let hash = &hashes[i];

        tracing::trace!(
            r"Message to L1 being sent:
|     hash     | {}
| from_address | {:#x}
|  to_address  | {:#x}
|   payload    | [{}]

",
            hash.as_str(),
            m.from_address,
            m.to_address,
            payload_str.join(", "));
    }
}

fn trace_l1_handler_tx_exec(tx: &L1HandlerTransaction) {
    // TODO: am I missing a simple way to print StarkFelt is hex..?
    let calldata_str: Vec<String> = tx.inner.calldata.0
        .iter()
        .map(|f| format!("{:#x}", FieldElement::from(*f))).collect();

    tracing::trace!(
        r"L1Handler transaction to be executed:
|      tx_hash     | {:#x}
| contract_address | {:#x}
|     selector     | {:#x}
|     calldata     | [{}]

",
        FieldElement::from(tx.inner.transaction_hash.0),
        FieldElement::from(*tx.inner.contract_address.0.key()),
        FieldElement::from(tx.inner.entry_point_selector.0),
        calldata_str.join(", "));
}
