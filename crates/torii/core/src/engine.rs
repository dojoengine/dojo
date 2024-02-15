use std::time::Duration;

use anyhow::Result;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{
    BlockId, BlockWithTxs, Event, InvokeTransaction, MaybePendingBlockWithTxs,
    MaybePendingTransactionReceipt, Transaction, TransactionReceipt,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Sender as BoundedSender;
use tokio::time::sleep;
use tracing::{error, info, trace, warn};

use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::sql::Sql;

pub struct Processors<P: Provider + Sync> {
    pub block: Vec<Box<dyn BlockProcessor<P>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<P>>>,
    pub event: Vec<Box<dyn EventProcessor<P>>>,
}

impl<P: Provider + Sync> Default for Processors<P> {
    fn default() -> Self {
        Self { block: vec![], event: vec![], transaction: vec![] }
    }
}

#[derive(Debug)]
pub struct EngineConfig {
    pub block_time: Duration,
    pub start_block: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { block_time: Duration::from_secs(1), start_block: 0 }
    }
}

pub struct Engine<'db, P: Provider + Sync> {
    world: WorldContractReader<P>,
    db: &'db mut Sql,
    provider: Box<P>,
    processors: Processors<P>,
    config: EngineConfig,
    shutdown_tx: Sender<()>,
    block_tx: Option<BoundedSender<u64>>,
}

struct UnprocessedEvent {
    keys: Vec<String>,
    data: Vec<String>,
}

impl<'db, P: Provider + Sync> Engine<'db, P> {
    pub fn new(
        world: WorldContractReader<P>,
        db: &'db mut Sql,
        provider: P,
        processors: Processors<P>,
        config: EngineConfig,
        shutdown_tx: Sender<()>,
        block_tx: Option<BoundedSender<u64>>,
    ) -> Self {
        Self { world, db, provider: Box::new(provider), processors, config, shutdown_tx, block_tx }
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut head = self.db.head().await?;
        if head == 0 {
            head = self.config.start_block;
        } else if self.config.start_block != 0 {
            warn!("start block ignored, stored head exists and will be used instead");
        }

        let mut backoff_delay = Duration::from_secs(1);
        let max_backoff_delay = Duration::from_secs(60);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break Ok(());
                }
                _ = async {
                    match self.sync_to_head(head).await {
                        Ok(latest_block_number) => {
                            head = latest_block_number;
                            backoff_delay = Duration::from_secs(1);
                        }
                        Err(e) => {
                            error!("getting  block: {}", e);
                            sleep(backoff_delay).await;
                            if backoff_delay < max_backoff_delay {
                                backoff_delay *= 2;
                            }
                        }
                    };
                    sleep(self.config.block_time).await;
                } => {}
            }
        }
    }

    pub async fn sync_to_head(&mut self, from: u64) -> Result<u64> {
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        if from < latest_block_number {
            // if `from` == 0, then the block may or may not be processed yet.
            let from = if from == 0 { from } else { from + 1 };
            self.sync_range(from, latest_block_number).await?;
        };

        Ok(latest_block_number)
    }

    pub async fn sync_range(&mut self, mut from: u64, to: u64) -> Result<()> {
        // Process all blocks from current to latest.
        while from <= to {
            let block_with_txs = match self.provider.get_block_with_txs(BlockId::Number(from)).await
            {
                Ok(block_with_txs) => block_with_txs,
                Err(e) => {
                    error!("getting block: {}", e);
                    continue;
                }
            };

            // send the current block number
            if let Some(ref block_tx) = self.block_tx {
                block_tx.send(from).await.expect("failed to send block number to gRPC server");
            }

            match self.process(block_with_txs).await {
                Ok(_) => {
                    self.db.set_head(from);
                    self.db.execute().await?;
                    from += 1;
                }
                Err(e) => {
                    error!("processing block: {}", e);
                    continue;
                }
            }
        }

        Ok(())
    }

    async fn process(&mut self, block: MaybePendingBlockWithTxs) -> Result<()> {
        let block: BlockWithTxs = match block {
            MaybePendingBlockWithTxs::Block(block) => block,
            _ => return Ok(()),
        };

        Self::process_block(self, &block).await?;

        for (tx_idx, transaction) in block.clone().transactions.iter().enumerate() {
            let transaction_hash = match transaction {
                Transaction::Invoke(invoke_transaction) => {
                    if let InvokeTransaction::V1(invoke_transaction) = invoke_transaction {
                        invoke_transaction.transaction_hash
                    } else {
                        continue;
                    }
                }
                Transaction::L1Handler(l1_handler_transaction) => {
                    l1_handler_transaction.transaction_hash
                }
                _ => continue,
            };

            self.process_transaction_and_receipt(transaction_hash, transaction, &block, tx_idx)
                .await?;
        }

        info!("processed block: {}", block.block_number);

        Ok(())
    }

    async fn process_transaction_and_receipt(
        &mut self,
        transaction_hash: FieldElement,
        transaction: &Transaction,
        block: &BlockWithTxs,
        tx_idx: usize,
    ) -> Result<()> {
        let receipt = match self.provider.get_transaction_receipt(transaction_hash).await {
            Ok(receipt) => match receipt {
                MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Invoke(receipt)) => {
                    Some(TransactionReceipt::Invoke(receipt))
                }
                MaybePendingTransactionReceipt::Receipt(TransactionReceipt::L1Handler(receipt)) => {
                    Some(TransactionReceipt::L1Handler(receipt))
                }
                _ => None,
            },
            Err(e) => {
                error!("getting transaction receipt: {}", e);
                return Err(e.into());
            }
        };

        if let Some(receipt) = receipt {
            let events = match &receipt {
                TransactionReceipt::Invoke(invoke_receipt) => &invoke_receipt.events,
                TransactionReceipt::L1Handler(l1_handler_receipt) => &l1_handler_receipt.events,
                _ => return Ok(()),
            };

            let mut world_event = false;
            for (event_idx, event) in events.iter().enumerate() {
                if event.from_address != self.world.address {
                    continue;
                }

                world_event = true;
                let event_id =
                    format!("0x{:064x}:0x{:04x}:0x{:04x}", block.block_number, tx_idx, event_idx);

                Self::process_event(self, block, &receipt, &event_id, event).await?;
            }

            if world_event {
                let transaction_id = format!("0x{:064x}:0x{:04x}", block.block_number, tx_idx);

                Self::process_transaction(self, block, &receipt, &transaction_id, transaction)
                    .await?;
            }
        }

        Ok(())
    }

    async fn process_block(&mut self, block: &BlockWithTxs) -> Result<()> {
        for processor in &self.processors.block {
            processor.process(self.db, self.provider.as_ref(), block).await?;
        }
        Ok(())
    }

    async fn process_transaction(
        &mut self,
        block: &BlockWithTxs,
        transaction_receipt: &TransactionReceipt,
        transaction_id: &str,
        transaction: &Transaction,
    ) -> Result<()> {
        for processor in &self.processors.transaction {
            processor
                .process(
                    self.db,
                    self.provider.as_ref(),
                    block,
                    transaction_receipt,
                    transaction,
                    transaction_id,
                )
                .await?
        }

        Ok(())
    }

    async fn process_event(
        &mut self,
        block: &BlockWithTxs,
        transaction_receipt: &TransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<()> {
        let transaction_hash = match transaction_receipt {
            TransactionReceipt::Invoke(invoke_receipt) => invoke_receipt.transaction_hash,
            TransactionReceipt::L1Handler(l1_handler_receipt) => {
                l1_handler_receipt.transaction_hash
            }
            _ => return Ok(()),
        };
        self.db.store_event(event_id, event, transaction_hash);
        for processor in &self.processors.event {
            if get_selector_from_name(&processor.event_key())? == event.keys[0]
                && processor.validate(event)
            {
                processor
                    .process(&self.world, self.db, block, transaction_receipt, event_id, event)
                    .await?;
            } else {
                let unprocessed_event = UnprocessedEvent {
                    keys: event.keys.iter().map(|k| format!("{:#x}", k)).collect(),
                    data: event.data.iter().map(|d| format!("{:#x}", d)).collect(),
                };

                trace!(
                    keys = ?unprocessed_event.keys,
                    data = ?unprocessed_event.data,
                    "unprocessed event",
                );
            }
        }
        Ok(())
    }
}
