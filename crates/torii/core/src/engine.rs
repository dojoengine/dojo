use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::time::Duration;

use anyhow::Result;
use dojo_world::contracts::world::WorldContractReader;
use hashlink::LinkedHashSet;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, Event, EventFilter, EventsPage, Felt,
    MaybePendingBlockWithTxHashes, Transaction, TransactionReceipt,
    TransactionReceiptWithBlockInfo,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Sender as BoundedSender;
use tokio::time::sleep;
use tracing::{error, info, trace, warn};

use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::sql::Sql;
use crate::types::ErcContract;

#[allow(missing_debug_implementations)]
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

#[allow(missing_debug_implementations)]
pub struct Engine<P: Provider + Sync> {
    world: WorldContractReader<P>,
    db: Sql,
    provider: Box<P>,
    processors: Processors<P>,
    config: EngineConfig,
    shutdown_tx: Sender<()>,
    block_tx: Option<BoundedSender<u64>>,
    // ERC tokens to index
    tokens: HashMap<Felt, ErcContract>,
}

struct UnprocessedEvent {
    keys: Vec<String>,
    data: Vec<String>,
}

impl<P: Provider + Sync> Engine<P> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world: WorldContractReader<P>,
        db: Sql,
        provider: P,
        processors: Processors<P>,
        config: EngineConfig,
        shutdown_tx: Sender<()>,
        block_tx: Option<BoundedSender<u64>>,
        tokens: HashMap<Felt, ErcContract>,
    ) -> Self {
        Self {
            world,
            db,
            provider: Box::new(provider),
            processors,
            config,
            shutdown_tx,
            block_tx,
            tokens,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut backoff_delay = Duration::from_secs(1);
        let max_backoff_delay = Duration::from_secs(60);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let mut erroring_out = false;
        self.initialize_cursors().await?;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break Ok(());
                }
                // TODO!: make this method cancel safe
                // see: https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety
                res = self.sync() => {
                    match res {
                        Ok(()) => {
                            if erroring_out {
                                erroring_out = false;
                                backoff_delay = Duration::from_secs(1);
                                info!(target: LOG_TARGET, "Syncing reestablished.");
                            }
                        }
                        Err(e) => {
                            erroring_out = true;
                            error!(target: LOG_TARGET, error = %e, "Syncing to head.");
                            sleep(backoff_delay).await;
                            if backoff_delay < max_backoff_delay {
                                backoff_delay *= 2;
                            }
                        }
                    };
                    sleep(self.config.block_time).await;
                }
            }
        }
    }

    // ASSUMPTION: events are ordered by (block number, transaction index)
    pub async fn sync(&mut self) -> Result<()> {
        // TODO: instead of fetching all of them at once, process them batch wise
        let mut events = Vec::new();
        let mut latest_for_contract = HashMap::new();

        let (world_events, block_number, transaction_hash) =
            self.get_contract_events(self.world.address).await?;
        latest_for_contract.insert(self.world.address, (block_number, transaction_hash));

        events.extend(world_events);

        for erc_address in self.tokens.keys() {
            let (erc_events, block_number, transaction_hash) =
                self.get_contract_events(*erc_address).await?;
            latest_for_contract.insert(*erc_address, (block_number, transaction_hash));
            events.extend(erc_events);
        }

        // Transactions & blocks to process
        let mut blocks = BTreeMap::new();
        let mut transactions = LinkedHashSet::new();

        for event in &events {
            let block_number = match event.block_number {
                Some(block_number) => block_number,
                // this means event is part of pending block
                None => latest_for_contract.get(&event.from_address).unwrap().0,
            };

            if let Entry::Vacant(e) = blocks.entry(block_number) {
                let block_id = if let Some(block_number) = event.block_number {
                    BlockId::Number(block_number)
                } else {
                    BlockId::Tag(BlockTag::Pending)
                };

                let block_timestamp = self.get_block_timestamp(block_id).await?;
                e.insert(block_timestamp);
            }

            if !transactions.contains(&(block_number, event.transaction_hash)) {
                transactions.insert((block_number, event.transaction_hash));
            }
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

        // Process block
        for (block_number, timestamp) in blocks.iter() {
            if let Some(ref block_tx) = self.block_tx {
                block_tx.send(*block_number).await?;
            }

            self.process_block(*block_number, *timestamp).await?;
        }

        for (contract_address, (block_number, transaction_hash)) in &latest_for_contract {
            self.db.set_head_and_latest_block_tx(
                *block_number,
                *transaction_hash,
                *contract_address,
            );
        }
        self.db.execute().await?;

        Ok(())
    }

    async fn get_block_timestamp(&self, block_id: BlockId) -> Result<u64> {
        match self.provider.get_block_with_tx_hashes(block_id).await? {
            MaybePendingBlockWithTxHashes::Block(block) => Ok(block.timestamp),
            MaybePendingBlockWithTxHashes::PendingBlock(block) => Ok(block.timestamp),
        }
    }

    // Process a transaction and its receipt.
    // We only call this for transactions which contains the event we are interested in.
    async fn process_transaction_and_receipt(
        &mut self,
        transaction_hash: Felt,
        transaction: &Transaction,
        block_number: u64,
        block_timestamp: u64,
    ) -> Result<()> {
        let receipt = self.provider.get_transaction_receipt(transaction_hash).await?;
        let events = match &receipt.receipt {
            TransactionReceipt::Invoke(receipt) => Some(&receipt.events),
            TransactionReceipt::L1Handler(receipt) => Some(&receipt.events),
            _ => None,
        };

        if let Some(events) = events {
            for (event_idx, event) in events.iter().enumerate() {
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

        Ok(())
    }

    async fn process_block(&mut self, block_number: u64, block_timestamp: u64) -> Result<()> {
        for processor in &self.processors.block {
            processor
                .process(&mut self.db, self.provider.as_ref(), block_number, block_timestamp)
                .await?
        }

        info!(target: LOG_TARGET, block_number = %block_number, "Processed block.");
        Ok(())
    }

    async fn process_transaction(
        &mut self,
        block_number: u64,
        block_timestamp: u64,
        transaction_receipt: &TransactionReceiptWithBlockInfo,
        transaction_hash: Felt,
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
        transaction_receipt: &TransactionReceiptWithBlockInfo,
        event_id: &str,
        event: &Event,
    ) -> Result<()> {
        self.db.store_event(
            event_id,
            event,
            *transaction_receipt.receipt.transaction_hash(),
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

    async fn get_contract_events(
        &self,
        contract_address: Felt,
    ) -> Result<(Vec<EmittedEvent>, u64, Option<Felt>)> {
        let (from, last_processed_tx, _) = self.db.head(contract_address).await?;
        let from = from.unwrap_or(0);

        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        let from_block = BlockId::Number(from);
        let to_block = if self.config.index_pending {
            BlockId::Tag(BlockTag::Pending)
        } else {
            BlockId::Number(latest_block_number)
        };

        let events_filter = EventFilter {
            from_block: Some(from_block),
            to_block: Some(to_block),
            address: Some(contract_address),
            keys: None,
        };

        let event_pages =
            get_all_events(&self.provider, events_filter, self.config.events_chunk_size).await?;
        let mut events = Vec::new();
        let mut last_processed_tx_cursor = last_processed_tx;

        for event_page in event_pages {
            for event in event_page.events {
                // Then we skip all transactions until we reach the last pending processed
                // transaction (if any)
                if let Some(tx) = last_processed_tx_cursor {
                    if event.transaction_hash != tx {
                        continue;
                    }
                    last_processed_tx_cursor = None;
                }

                // Skip the latest pending block transaction events
                // * as we might have multiple events for the same transaction
                if let Some(tx) = last_processed_tx {
                    if event.transaction_hash == tx {
                        continue;
                    }
                }

                events.push(event);
            }
        }

        let (last_block, last_tx) = if let Some(last_event) = events.last() {
            (
                last_event.block_number.unwrap_or_else(|| latest_block_number + 1),
                Some(last_event.transaction_hash),
            )
        } else {
            (latest_block_number + 1, None)
        };

        Ok((events, last_block, last_tx))
    }

    async fn initialize_cursors(&mut self) -> Result<()> {
        let (from, _, _) = self.db.head(self.world.address).await?;

        if from.is_none() {
            self.db.set_head(self.config.start_block, self.world.address);
        } else if self.config.start_block != 0 {
            warn!(target: LOG_TARGET, "Start block for worldignored, stored head exists and will be used instead.");
        }

        for contract in self.tokens.values() {
            let (from, _, _) = self.db.head(contract.contract_address).await?;
            if from.is_none() {
                self.db.set_head(contract.start_block, contract.contract_address);
            } else if contract.start_block != 0 {
                warn!(target: LOG_TARGET, "Start block for contract {:#x} ignored, stored head exists and will be used instead.", contract.contract_address);
            }
        }

        Ok(())
    }
}

async fn get_all_events<P>(
    provider: &P,
    events_filter: EventFilter,
    events_chunk_size: u64,
) -> Result<Vec<EventsPage>>
where
    P: Provider + Sync,
{
    let mut events_pages = Vec::new();
    let mut continuation_token = None;

    loop {
        let events_page = provider
            .get_events(events_filter.clone(), continuation_token.clone(), events_chunk_size)
            .await?;

        continuation_token = events_page.continuation_token.clone();
        events_pages.push(events_page);

        if continuation_token.is_none() {
            break;
        }
    }

    Ok(events_pages)
}
