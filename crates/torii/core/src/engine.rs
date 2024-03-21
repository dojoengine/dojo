use std::time::Duration;

use anyhow::Result;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{
    BlockId, EmittedEvent, Event, EventFilter, MaybePendingBlockWithTxHashes,
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
    pub events_chunk_size: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { block_time: Duration::from_secs(1), start_block: 0, events_chunk_size: 1000 }
    }
}

pub struct Engine<P: Provider + Sync> {
    world: WorldContractReader<P>,
    db: Sql,
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

impl<P: Provider + Sync> Engine<P> {
    pub fn new(
        world: WorldContractReader<P>,
        db: Sql,
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

    pub async fn sync_range(&mut self, from: u64, to: u64) -> Result<()> {
        // Process all blocks from current to latest.
        let get_events = |token: Option<String>| {
            self.provider.get_events(
                EventFilter {
                    from_block: Some(BlockId::Number(from)),
                    to_block: Some(BlockId::Number(to)),
                    address: Some(self.world.address),
                    keys: None,
                },
                token,
                self.config.events_chunk_size,
            )
        };

        // handle next events pages
        let mut events_pages = vec![get_events(None).await?];

        while let Some(token) = &events_pages.last().unwrap().continuation_token {
            events_pages.push(get_events(Some(token.clone())).await?);
        }

        let mut last_block: u64 = 0;
        for events_page in events_pages {
            for event in events_page.events {
                self.process(event, &mut last_block).await?;
            }
        }

        self.db.execute().await?;

        Ok(())
    }

    async fn get_block_timestamp(&self, block_number: u64) -> Result<u64> {
        match self.provider.get_block_with_tx_hashes(BlockId::Number(block_number)).await? {
            MaybePendingBlockWithTxHashes::Block(block) => Ok(block.timestamp),
            MaybePendingBlockWithTxHashes::PendingBlock(block) => Ok(block.timestamp),
        }
    }

    async fn process(&mut self, event: EmittedEvent, last_block: &mut u64) -> Result<()> {
        let block_number = match event.block_number {
            Some(block_number) => block_number,
            None => {
                error!("event without block number");
                return Ok(());
            }
        };
        let block_timestamp = self.get_block_timestamp(block_number).await?;

        if block_number > *last_block {
            *last_block = block_number;

            if let Some(ref block_tx) = self.block_tx {
                block_tx.send(block_number).await?;
            }

            Self::process_block(self, block_number, block_timestamp, event.block_hash.unwrap())
                .await?;
            info!(target: "torii_core::engine", block_number = %block_number, "Processed block");

            self.db.set_head(block_number);
        }

        let transaction = self.provider.get_transaction_by_hash(event.transaction_hash).await?;
        self.process_transaction_and_receipt(
            event.transaction_hash,
            &transaction,
            block_number,
            block_timestamp,
        )
        .await?;

        Ok(())
    }

    async fn process_transaction_and_receipt(
        &mut self,
        transaction_hash: FieldElement,
        transaction: &Transaction,
        block_number: u64,
        block_timestamp: u64,
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
                    format!("{:#064x}:{:#x}:{:#04x}", block_number, transaction_hash, event_idx);

                Self::process_event(
                    self,
                    block_number,
                    block_timestamp,
                    &receipt,
                    &event_id,
                    event,
                )
                .await?;
            }

            if world_event {
                Self::process_transaction(
                    self,
                    block_number,
                    block_timestamp,
                    &receipt,
                    transaction_hash,
                    transaction,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn process_block(
        &mut self,
        block_number: u64,
        block_timestamp: u64,
        block_hash: FieldElement,
    ) -> Result<()> {
        for processor in &self.processors.block {
            processor
                .process(
                    &mut self.db,
                    self.provider.as_ref(),
                    block_number,
                    block_timestamp,
                    block_hash,
                )
                .await?;
        }
        Ok(())
    }

    async fn process_transaction(
        &mut self,
        block_number: u64,
        block_timestamp: u64,
        transaction_receipt: &TransactionReceipt,
        transaction_hash: FieldElement,
        transaction: &Transaction,
    ) -> Result<()> {
        for processor in &self.processors.transaction {
            processor
                .process(
                    &mut self.db,
                    self.provider.as_ref(),
                    block_number,
                    block_timestamp,
                    transaction_receipt,
                    transaction_hash,
                    transaction,
                )
                .await?
        }

        Ok(())
    }

    async fn process_event(
        &mut self,
        block_number: u64,
        block_timestamp: u64,
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
        self.db.store_event(event_id, event, transaction_hash, block_timestamp);
        for processor in &self.processors.event {
            // If the processor has no event_key, means it's a catch-all processor.
            // We also validate the event
            if (processor.event_key().is_empty()
                || get_selector_from_name(&processor.event_key())? == event.keys[0])
                && processor.validate(event)
            {
                processor
                    .process(
                        &self.world,
                        &mut self.db,
                        block_number,
                        block_timestamp,
                        transaction_receipt,
                        event_id,
                        event,
                    )
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
