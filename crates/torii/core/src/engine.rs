use std::collections::BTreeMap;
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
    pub index_pending: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            block_time: Duration::from_secs(1),
            start_block: 0,
            events_chunk_size: 1000,
            index_pending: false,
        }
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
        let (mut head, mut pending_block_tx) = self.db.head().await?;
        if head == 0 {
            head = self.config.start_block;
        } else if self.config.start_block != 0 {
            warn!(target: LOG_TARGET, "Start block ignored, stored head exists and will be used instead.");
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
                    match self.sync_to_head(head, pending_block_tx).await {
                        Ok((latest_block_number, latest_pending_tx)) => {
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
    ) -> Result<(u64, Option<FieldElement>)> {
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        if from < latest_block_number {
            // if `from` == 0, then the block may or may not be processed yet.
            let from = if from == 0 { from } else { from + 1 };
            pending_block_tx = self.sync_range(from, latest_block_number, pending_block_tx).await?;
        } else if self.config.index_pending {
            pending_block_tx = self.sync_pending(latest_block_number + 1, pending_block_tx).await?;
        }

        Ok((latest_block_number, pending_block_tx))
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
                    *transaction.transaction_hash(),
                    &transaction,
                    block_number,
                    block.timestamp,
                )
                .await
            {
                Err(e) => {
                    match e.to_string().as_str() {
                        "TransactionHashNotFound" => {
                            // We failed to fetch the transaction, which is because
                            // the transaction might not have passed the validation stage.
                            // So we can safely ignore this transaction and not process it, as it
                            // rejected.
                            warn!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction.transaction_hash()), "Ignored failed pending transaction.");
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
        pending_block_tx: Option<FieldElement>,
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

        // Transactions & blocks to process
        let mut last_block = 0_u64;
        let mut blocks = BTreeMap::new();

        // Flatten events pages and events according to the pending block cursor
        // to array of (block_number, transaction_hash)
        let mut pending_block_tx_cursor = pending_block_tx;
        let mut transactions = vec![];
        for events_page in &events_pages {
            for event in &events_page.events {
                let block_number = match event.block_number {
                    Some(block_number) => block_number,
                    // If the block number is not present, try to fetch it from the transaction
                    // receipt Should not/rarely happen. Thus the additional
                    // fetch is acceptable.
                    None => {
                        match self.provider.get_transaction_receipt(event.transaction_hash).await? {
                            MaybePendingTransactionReceipt::Receipt(
                                TransactionReceipt::Invoke(receipt),
                            ) => receipt.block_number,
                            MaybePendingTransactionReceipt::Receipt(
                                TransactionReceipt::L1Handler(receipt),
                            ) => receipt.block_number,
                            // If it's a pending transaction, we assume the block number is the
                            // latest + 1
                            _ => to + 1,
                        }
                    }
                };

                // Keep track of last block number and fetch block timestamp
                if block_number > last_block {
                    let block_timestamp = self.get_block_timestamp(block_number).await?;
                    blocks.insert(block_number, block_timestamp);

                    last_block = block_number;
                }

                // Then we skip all transactions until we reach the last pending processed
                // transaction (if any)
                if let Some(tx) = pending_block_tx_cursor {
                    if event.transaction_hash != tx {
                        continue;
                    }

                    pending_block_tx_cursor = None;
                }

                // Skip the latest pending block transaction events
                // * as we might have multiple events for the same transaction
                if let Some(tx) = pending_block_tx {
                    if event.transaction_hash == tx {
                        continue;
                    }
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

        // Process blocks
        for (block_number, block_timestamp) in blocks.iter() {
            if let Some(ref block_tx) = self.block_tx {
                block_tx.send(*block_number).await?;
            }

            self.process_block(*block_number, *block_timestamp).await?;
            info!(target: LOG_TARGET, block_number = %block_number, "Processed block.");
        }

        // Process all transactions
        for (block_number, transaction_hash) in transactions {
            // Process transaction
            let transaction = self.provider.get_transaction_by_hash(transaction_hash).await?;

            self.process_transaction_and_receipt(
                transaction_hash,
                &transaction,
                block_number,
                blocks[&block_number],
            )
            .await?;
        }

        // We return None for the pending_block_tx because our sync_range
        // retrieves only specific events from the world. so some transactions
        // might get ignored and wont update the cursor.
        // so once the sync range is done, we assume all of the tx of the block
        // have been processed.

        self.db.set_head(to, None);

        self.db.execute().await?;

        Ok(None)
    }

    async fn get_block_timestamp(&self, block_number: u64) -> Result<u64> {
        match self.provider.get_block_with_tx_hashes(BlockId::Number(block_number)).await? {
            MaybePendingBlockWithTxHashes::Block(block) => Ok(block.timestamp),
            MaybePendingBlockWithTxHashes::PendingBlock(block) => Ok(block.timestamp),
        }
    }

    async fn process_transaction_and_receipt(
        &mut self,
        transaction_hash: FieldElement,
        transaction: &Transaction,
        block_number: u64,
        block_timestamp: u64,
    ) -> Result<()> {
        let receipt = self.provider.get_transaction_receipt(transaction_hash).await?;
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
                    transaction_hash,
                    transaction,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn process_block(&mut self, block_number: u64, block_timestamp: u64) -> Result<()> {
        for processor in &self.processors.block {
            processor
                .process(&mut self.db, self.provider.as_ref(), block_number, block_timestamp)
                .await?
        }
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
                if let Err(e) = processor
                    .process(
                        &self.world,
                        &mut self.db,
                        block_number,
                        block_timestamp,
                        transaction_receipt,
                        event_id,
                        event,
                    )
                    .await
                {
                    error!(target: LOG_TARGET, event_name = processor.event_key(), error = %e, "Processing event.");
                }
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
