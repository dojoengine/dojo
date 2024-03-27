use std::collections::HashMap;
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
use crate::provider::provider::{KatanaProvider, TransactionsPageCursor};
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

pub struct Engine<P: KatanaProvider + Sync, R: Provider + Sync> {
    world: WorldContractReader<R>,
    db: Sql,
    provider: Box<P>,
    processors: Processors<R>,
    config: EngineConfig,
    shutdown_tx: Sender<()>,
    block_tx: Option<BoundedSender<u64>>,
    processed_blocks: HashMap<u64, Block>,
}

struct UnprocessedEvent {
    keys: Vec<String>,
    data: Vec<String>,
}

#[derive(Debug, Clone)]
struct Block {
    /// The hash of this block's parent
    pub _parent_hash: FieldElement,
    /// The block number (its height)
    pub block_number: u64,
    /// The time in which the block was created, encoded in Unix time
    pub timestamp: u64,
}

impl<P: KatanaProvider + Sync, R: Provider + Sync> Engine<P, R> {
    pub fn new(
        world: WorldContractReader<R>,
        db: Sql,
        provider: P,
        processors: Processors<R>,
        config: EngineConfig,
        shutdown_tx: Sender<()>,
        block_tx: Option<BoundedSender<u64>>,
    ) -> Self {
        Self {
            world,
            db,
            provider: Box::new(provider),
            processors,
            config,
            shutdown_tx,
            block_tx,
            processed_blocks: HashMap::new(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut head = self.db.head().await?;
        if head == 0 {
            head = self.config.start_block;
        } else if self.config.start_block != 0 {
            warn!("start block ignored, stored head exists and will be used instead");
        }

        // Fetch the first page of transactions to determine if the provider supports katana.
        // If yes, we process and store the transactions page.
        // And use the returned cursor for next pages.
        let transactions_page = match self
            .provider
            .get_transactions(TransactionsPageCursor {
                block_number: head,
                transaction_index: 0,
                chunk_size: 100,
            })
            .await
        {
            Ok(page) => Some(page),
            Err(err) => {
                info!("provider does not support katana, fetching events instead: {}", err);
                None
            }
        };

        // Process the first page of transactions.
        // For katana providers.
        if let Some(transactions_page) = transactions_page.clone() {
            self.process_katana(
                transactions_page.transactions,
                head,
                transactions_page.cursor.block_number - 1,
            )
            .await?;

            self.db.execute().await?;
        }

        let mut current_cursor = transactions_page.clone().map(|t| t.cursor);

        let mut backoff_delay = Duration::from_secs(1);
        let max_backoff_delay = Duration::from_secs(60);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break Ok(());
                }
                _ = async {
                    match self.sync_to_head(head, current_cursor.clone()).await {
                        Ok((latest_block_number, cursor)) => {
                            if let Some(cursor) = cursor {
                                current_cursor = Some(cursor);
                            }
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

    pub async fn sync_to_head(
        &mut self,
        from: u64,
        cursor: Option<TransactionsPageCursor>,
    ) -> Result<(u64, Option<TransactionsPageCursor>)> {
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        let mut new_cursor = None;
        if from < latest_block_number {
            // if `from` == 0, then the block may or may not be processed yet.
            let from = if from == 0 { from } else { from + 1 };

            if let Some(cursor) = cursor {
                // we fetch pending block too
                new_cursor = Some(self.sync_range_katana(&cursor).await?);
            } else {
                self.sync_range(from, latest_block_number).await?;
            }
        };

        Ok((latest_block_number, new_cursor))
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

    async fn sync_range_katana(
        &mut self,
        cursor: &TransactionsPageCursor,
    ) -> Result<TransactionsPageCursor> {
        let transactions = self.provider.get_transactions(cursor.clone()).await?;

        println!("transactions: {:?}", transactions);

        self.process_katana(
            transactions.transactions,
            cursor.block_number,
            transactions.cursor.block_number - 1,
        )
        .await?;
        self.db.execute().await?;

        Ok(transactions.cursor)
    }

    async fn get_block_metadata(&self, block_number: u64) -> Result<Block> {
        match self.provider.get_block_with_tx_hashes(BlockId::Number(block_number)).await? {
            MaybePendingBlockWithTxHashes::Block(block) => Ok(Block {
                block_number: block.block_number,
                _parent_hash: block.parent_hash,
                timestamp: block.timestamp,
            }),
            MaybePendingBlockWithTxHashes::PendingBlock(block) => Ok(Block {
                block_number,
                _parent_hash: block.parent_hash,
                timestamp: block.timestamp,
            }),
        }
    }

    async fn process_katana(
        &mut self,
        transactions: Vec<(Transaction, MaybePendingTransactionReceipt)>,
        from: u64,
        to: u64,
    ) -> Result<()> {
        for block_number in from..=to {
            if let Some(ref block_tx) = self.block_tx {
                block_tx.send(block_number).await?;
            }

            let block = self.get_block_metadata(block_number).await?;
            self.process_block(&block).await?;
        }

        self.db.set_head(to);

        for (transaction, receipt) in transactions {
            let block_number = match &receipt {
                MaybePendingTransactionReceipt::Receipt(receipt) => match receipt {
                    TransactionReceipt::Invoke(receipt) => receipt.block_number,
                    TransactionReceipt::L1Handler(receipt) => receipt.block_number,
                    TransactionReceipt::Declare(receipt) => receipt.block_number,
                    TransactionReceipt::Deploy(receipt) => receipt.block_number,
                    TransactionReceipt::DeployAccount(receipt) => receipt.block_number,
                },
                // If the receipt is pending, we can assume that the transaction
                // block number is the heighest block number we have processed.
                MaybePendingTransactionReceipt::PendingReceipt(_) => to,
            };
            let block_timestamp = match self.processed_blocks.get(&block_number) {
                Some(block) => block.timestamp,
                None => {
                    return Err(anyhow::anyhow!(
                        "block {} not found in processed blocks",
                        block_number
                    ));
                }
            };

            self.process_transaction_and_receipt(
                &transaction,
                Some(receipt),
                block_number,
                block_timestamp,
            )
            .await?;
        }

        Ok(())
    }

    async fn process(&mut self, event: EmittedEvent, last_block: &mut u64) -> Result<()> {
        let block_number = match event.block_number {
            Some(block_number) => block_number,
            None => {
                let error = anyhow::anyhow!("event has no block number");
                error!("processing event: {}", error);

                return Err(error);
            }
        };
        let block = self.get_block_metadata(block_number).await?;

        if block_number > *last_block {
            *last_block = block_number;

            if let Some(ref block_tx) = self.block_tx {
                block_tx.send(block_number).await?;
            }

            self.process_block(&block).await?;

            self.db.set_head(block_number);
        }

        let transaction = self.provider.get_transaction_by_hash(event.transaction_hash).await?;
        self.process_transaction_and_receipt(&transaction, None, block_number, block.timestamp)
            .await?;

        Ok(())
    }

    async fn process_transaction_and_receipt(
        &mut self,
        transaction: &Transaction,
        receipt: Option<MaybePendingTransactionReceipt>,
        block_number: u64,
        block_timestamp: u64,
    ) -> Result<()> {
        let transaction_hash = transaction.transaction_hash();

        let receipt = receipt.unwrap_or(
            match self.provider.get_transaction_receipt(transaction_hash).await {
                Ok(receipt) => receipt,
                Err(e) => {
                    error!("getting transaction receipt: {}", e);
                    return Err(e.into());
                }
            },
        );

        let receipt = match receipt {
            MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Invoke(receipt)) => {
                Some(TransactionReceipt::Invoke(receipt.clone()))
            }
            MaybePendingTransactionReceipt::Receipt(TransactionReceipt::L1Handler(receipt)) => {
                Some(TransactionReceipt::L1Handler(receipt.clone()))
            }
            _ => None,
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
                    transaction_hash.clone(),
                    transaction,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn process_block(&mut self, block: &Block) -> Result<()> {
        for processor in &self.processors.block {
            processor
                .process(&mut self.db, &self.world.provider, block.block_number, block.timestamp)
                .await?;
        }

        self.processed_blocks.insert(block.block_number, block.clone());
        info!(target: "torii_core::engine", block_number = %block.block_number, "Processed block");

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
                    &self.world.provider,
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
