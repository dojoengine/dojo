use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{
    BlockId, BlockTag, Event, EventFilter, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    MaybePendingTransactionReceipt, PendingTransactionReceipt, Transaction, TransactionReceipt,
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

pub(crate) const LOG_TARGET: &str = "tori_core::engine";

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
        let (mut head, mut pending_block_tx) = self.db.head().await?;
        if head == 0 {
            head = self.config.start_block;
        } else if self.config.start_block != 0 {
            warn!(target: LOG_TARGET, "Start block ignored, stored head exists and will be used instead.");
        }

        // Sync the first page of transactions to determine if the provider supports katana.
        // If yes, we process and store the transactions page.
        // And use the returned cursor for next pages.
        let mut cursor = match self
            .sync_range_katana(&TransactionsPageCursor {
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

        let mut backoff_delay = Duration::from_secs(1);
        let max_backoff_delay = Duration::from_secs(60);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break Ok(());
                }
                _ = async {
                    match self.sync_to_head(head, pending_block_tx, cursor.clone()).await {
                        Ok((latest_block_number, latest_pending_tx, new_cursor)) => {
                            if let Some(_) = new_cursor {
                                cursor = new_cursor;
                            }

                            pending_block_tx = latest_pending_tx;
                            head = latest_block_number;
                            backoff_delay = Duration::from_secs(1);
                        }
                        Err(e) => {
                            error!(target: LOG_TARGET, error = %e, "Syncing to head.");
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
        mut pending_block_tx: Option<FieldElement>,
        mut katana_cursor: Option<TransactionsPageCursor>,
    ) -> Result<(u64, Option<FieldElement>, Option<TransactionsPageCursor>)> {
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        // katana sync
        if let Some(cursor) = katana_cursor {
            katana_cursor = Some(self.sync_range_katana(&cursor).await?);
        } else {
            // default sync
            if from < latest_block_number {
                // if `from` == 0, then the block may or may not be processed yet.
                let from = if from == 0 { from } else { from + 1 };
                pending_block_tx = self.sync_range(from, latest_block_number, pending_block_tx).await?;
            } else {
                // pending block sync
                pending_block_tx = self.sync_pending(latest_block_number + 1, pending_block_tx).await?;
            }    
        }

        
        Ok((latest_block_number, pending_block_tx, katana_cursor))
    }

    pub async fn sync_pending(
        &mut self,
        block_number: u64,
        mut pending_block_tx: Option<FieldElement>,
    ) -> Result<Option<FieldElement>> {
        let block = if let MaybePendingBlockWithTxs::PendingBlock(pending) =
            self.provider.get_block_with_txs(BlockId::Tag(BlockTag::Pending)).await?
        {
            pending
        } else {
            return Ok(None);
        };

        // Skip transactions that have been processed already
        // Our cursor is the last processed transaction
        let mut pending_block_tx_cursor = pending_block_tx;
        for transaction in block.transactions {
            if let Some(tx) = pending_block_tx_cursor {
                if transaction.transaction_hash() != &tx {
                    continue;
                }

                pending_block_tx_cursor = None;
                continue;
            }

            match self
                .process_transaction_and_receipt(
                    &transaction,
                    None,
                    block_number,
                    block.timestamp,
                )
                .await
            {
                Err(e) => {
                    match e.to_string().as_str() {
                        "TransactionHashNotFound" => {
                            warn!(target: LOG_TARGET, error = %e, transaction_hash = %format!("{:#x}", transaction.transaction_hash()), "Processing pending transaction.");
                            // We failed to fetch the transaction, which might be due to us indexing
                            // the pending transaction too fast. We will
                            // fail silently and retry processing the transaction in the next
                            // iteration.
                            return Ok(pending_block_tx);
                        }
                        _ => {
                            error!(target: LOG_TARGET, error = %e, transaction_hash = %format!("{:#x}", transaction.transaction_hash()), "Processing pending transaction.");
                            return Err(e);
                        }
                    }
                }
                Ok(_) => {
                    info!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction.transaction_hash()), "Processed pending transaction.")
                }
            }

            pending_block_tx = Some(*transaction.transaction_hash());
        }

        // Set the head to the last processed pending transaction
        // Head block number should still be latest block number
        self.db.set_head(block_number - 1, pending_block_tx);

        self.db.execute().await?;
        Ok(pending_block_tx)
    }

    pub async fn sync_range(
        &mut self,
        from: u64,
        to: u64,
        mut pending_block_tx: Option<FieldElement>,
    ) -> Result<Option<FieldElement>> {
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
        
        // Flatten events pages and events according to the pending block cursor
        // to array of (block_number, transaction_hash)
        let mut transactions = vec![];
        for events_page in &events_pages {
            for event in &events_page.events {
                let block_number = match event.block_number {
                    Some(block_number) => block_number,
                    None => return Err(anyhow::anyhow!("Event without block number.")),
                };

                // Keep track of last block number and fetch block timestamp
                if let None = self.processed_blocks.get(&block_number) {
                    let block = self.get_block_metadata(block_number).await?;

                    if let Some(ref block_tx) = self.block_tx {
                        block_tx.send(block.block_number).await?;
                    }
        
                    self.process_block(&block).await?;
                    info!(target: LOG_TARGET, block_number = %block_number, "Processed block.");
                    
                }

                // Then we skip all transactions until we reach the last pending processed
                // transaction (if any)
                if let Some(tx) = pending_block_tx {
                    if event.transaction_hash != tx {
                        continue;
                    }

                    // Then we skip that processed transaction
                    pending_block_tx = None;
                    continue;
                }

                if let Some((_, last_tx_hash)) = transactions.last() {
                    // Dedup transactions
                    // As me might have multiple events for the same transaction
                    if *last_tx_hash == event.transaction_hash {
                        continue;
                    }
                }
                transactions.push((block_number, event.transaction_hash));
            }
        }

        // Process all transactions
        for (block_number, transaction_hash) in transactions {
            // Process transaction
            let transaction = self.provider.get_transaction_by_hash(transaction_hash).await?;

            self.process_transaction_and_receipt(
                &transaction,
                None,
                block_number,
                self.processed_blocks[&block_number].timestamp,
            )
            .await?;
        }

        self.db.set_head(to, pending_block_tx);

        self.db.execute().await?;

        Ok(pending_block_tx)
    }

    async fn sync_range_katana(
        &mut self,
        cursor: &TransactionsPageCursor,
    ) -> Result<TransactionsPageCursor> {
        let transactions = self.provider.get_transactions(cursor.clone()).await?;

        let (from, to) = (cursor.block_number, transactions.cursor.block_number-1);
        for block_number in from..=to {
            if let Some(ref block_tx) = self.block_tx {
                block_tx.send(block_number).await?;
            }

            let block = self.get_block_metadata(block_number).await?;
            self.process_block(&block).await?;
        }

        self.db.set_head(to, None);

        for (transaction, receipt) in transactions.transactions {
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

    async fn process_transaction_and_receipt(
        &mut self,
        transaction: &Transaction,
        receipt: Option<MaybePendingTransactionReceipt>,
        block_number: u64,
        block_timestamp: u64,
    ) -> Result<()> {
        let transaction_hash = transaction.transaction_hash();
        let receipt = receipt.unwrap_or(
            self.provider
                .get_transaction_receipt(transaction_hash)
                .await?,
        );

        let events = match &receipt {
            MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Invoke(receipt)) => {
                Some(&receipt.events)
            }
            MaybePendingTransactionReceipt::Receipt(TransactionReceipt::L1Handler(receipt)) => {
                Some(&receipt.events)
            }
            MaybePendingTransactionReceipt::PendingReceipt(PendingTransactionReceipt::Invoke(
                receipt,
            )) => Some(&receipt.events),
            MaybePendingTransactionReceipt::PendingReceipt(
                PendingTransactionReceipt::L1Handler(receipt),
            ) => Some(&receipt.events),
            _ => None,
        };

        if let Some(events) = events {
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
        transaction_receipt: &MaybePendingTransactionReceipt,
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
        transaction_receipt: &MaybePendingTransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<()> {
        self.db.store_event(
            event_id,
            event,
            *transaction_receipt.transaction_hash(),
            block_timestamp,
        );
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
                    target: LOG_TARGET,
                    keys = ?unprocessed_event.keys,
                    data = ?unprocessed_event.data,
                    "Unprocessed event.",
                );
            }
        }
        Ok(())
    }
}
