mod ethereum_messenger;
mod starknet_messenger;
mod any_messenger;

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::time;
use starknet::core::types::MsgToL1;
use ethers::providers::ProviderError;

use crate::sequencer::SequencerMessagingConfig;
use crate::backend::{Backend, storage::transaction::Transaction}
;
use any_messenger::AnyMessenger;

type MessengerResult<T> = Result<T, MessengerError>;

#[derive(Debug, thiserror::Error)]
pub enum MessengerError {
    #[error("Error initializing messaging.")]
    InitError,
    #[error("Error gathering messages.")]
    GatherError,
    #[error("Error sending messages.")]
    SendError,
    #[error("Error ethereum provider: {0}.")]
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
    async fn gather_messages(&self, from_block: u64, max_blocks: u64) -> MessengerResult<(u64, Vec<Transaction>)>;

    /// Computes the hash of the given messages and sends them to the settlement chain.
    ///
    /// Once message's hash is settled, one must send a transaction (with the message content)
    /// on the settlement chain to actually consume it.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to settle.
    async fn settle_messages(&self, messages: &Vec<MsgToL1>) -> MessengerResult<()>;

    /// Sends a transaction to the settlement chain using the message content to define
    /// the recipient and the calldata.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to execute.
    async fn execute_messages(&self, messages: &Vec<MsgToL1>) -> MessengerResult<()>;
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
            = worker.send_messages(local_latest_block_number).await?;

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
    async fn send_messages(&self, from_block: u64) -> MessengerResult<(u64, u64)> {
        let local_latest = self.starknet.storage.read().await.latest_number;

        if from_block > local_latest {
            return Ok((from_block, 0));
        }

        let messages_count = 0;

        for i in from_block..local_latest {
            if let Some(block) = self.starknet.storage.read().await.block_by_number(i) {
                for o in &block.outputs {
                    match self.messenger.settle_messages(&o.messages_sent).await {
                        Ok(_) => (),
                        Err(e) => tracing::warn!("Error settling messages for block {}: {:?}", i, e),
                    };
                }   
            }
        }

        tracing::debug!("Messages sent {} [{} - {}]", messages_count, from_block, local_latest);
        Ok((local_latest, messages_count))
    }

    /// Fetches messages from the settlement chain.
    /// Returns the latest fetched block, and the count of messages gathered.
    async fn gather_messages(&self, from_block: u64, max_blocks: u64) -> (u64, u64) {
        if let Ok((latest_block_fetched, l1_handler_txs))
            = self.messenger.gather_messages(from_block, max_blocks).await
        {
            for tx in &l1_handler_txs {
                self.starknet.handle_transaction(tx.clone()).await;
            }
            
            tracing::debug!(
                "Messages gathered {} [{} - {}]",
                l1_handler_txs.len(),
                from_block,
                latest_block_fetched
            );
            
            return (latest_block_fetched + 1, l1_handler_txs.len() as u64)
        } else {
            (from_block, 0)
        }
    }
}
