use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bitflags::bitflags;
use dojo_world::contracts::world::WorldContractReader;
use futures_util::future::try_join_all;
use hashlink::LinkedHashMap;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, Event, EventFilter, Felt, MaybePendingBlockWithReceipts,
    MaybePendingBlockWithTxHashes, PendingBlockWithReceipts, ReceiptBlock, Transaction,
    TransactionReceipt, TransactionReceiptWithBlockInfo, TransactionWithReceipt,
};
use starknet::providers::Provider;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Sender as BoundedSender;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Instant};
use tracing::{debug, error, info, trace, warn};

use crate::processors::event_message::EventMessageProcessor;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::sql::Sql;

#[allow(missing_debug_implementations)]
pub struct Processors<P: Provider + Send + Sync + std::fmt::Debug + 'static> {
    pub block: Vec<Box<dyn BlockProcessor<P>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<P>>>,
    pub event: HashMap<Felt, Arc<dyn EventProcessor<P>>>,
    pub catch_all_event: Box<dyn EventProcessor<P>>,
}

impl<P: Provider + Send + Sync + std::fmt::Debug + 'static> Default for Processors<P> {
    fn default() -> Self {
        Self {
            block: vec![],
            event: HashMap::new(),
            transaction: vec![],
            catch_all_event: Box::new(EventMessageProcessor) as Box<dyn EventProcessor<P>>,
        }
    }
}

pub(crate) const LOG_TARGET: &str = "torii_core::engine";
pub const QUERY_QUEUE_BATCH_SIZE: usize = 1000;

bitflags! {
    #[derive(Debug, Clone)]
    pub struct IndexingFlags: u32 {
        const TRANSACTIONS = 0b00000001;
        const RAW_EVENTS = 0b00000010;
    }
}

#[derive(Debug)]
pub struct EngineConfig {
    pub polling_interval: Duration,
    pub start_block: u64,
    pub events_chunk_size: u64,
    pub index_pending: bool,
    pub max_concurrent_tasks: usize,
    pub flags: IndexingFlags,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_millis(500),
            start_block: 0,
            events_chunk_size: 1024,
            index_pending: true,
            max_concurrent_tasks: 100,
            flags: IndexingFlags::empty(),
        }
    }
}

#[derive(Debug)]
pub enum FetchDataResult {
    Range(FetchRangeResult),
    Pending(FetchPendingResult),
    None,
}

#[derive(Debug)]
pub struct FetchRangeResult {
    // (block_number, transaction_hash) -> events
    pub transactions: LinkedHashMap<(u64, Felt), Vec<EmittedEvent>>,
    pub blocks: BTreeMap<u64, u64>,
    pub latest_block_number: u64,
}

#[derive(Debug)]
pub struct FetchPendingResult {
    pub pending_block: Box<PendingBlockWithReceipts>,
    pub last_pending_block_tx: Option<Felt>,
    pub block_number: u64,
}

#[derive(Debug)]
pub struct ParallelizedEvent {
    pub block_number: u64,
    pub block_timestamp: u64,
    pub event_id: String,
    pub event: Event,
}

#[allow(missing_debug_implementations)]
pub struct Engine<P: Provider + Send + Sync + std::fmt::Debug + 'static> {
    world: Arc<WorldContractReader<P>>,
    db: Sql,
    provider: Box<P>,
    processors: Arc<Processors<P>>,
    config: EngineConfig,
    shutdown_tx: Sender<()>,
    block_tx: Option<BoundedSender<u64>>,
    tasks: HashMap<u64, Vec<ParallelizedEvent>>,
}

struct UnprocessedEvent {
    keys: Vec<String>,
    data: Vec<String>,
}

impl<P: Provider + Send + Sync + std::fmt::Debug + 'static> Engine<P> {
    pub fn new(
        world: WorldContractReader<P>,
        db: Sql,
        provider: P,
        processors: Processors<P>,
        config: EngineConfig,
        shutdown_tx: Sender<()>,
        block_tx: Option<BoundedSender<u64>>,
    ) -> Self {
        Self {
            world: Arc::new(world),
            db,
            provider: Box::new(provider),
            processors: Arc::new(processors),
            config,
            shutdown_tx,
            block_tx,
            tasks: HashMap::new(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        // use the start block provided by user if head is 0
        let (head, _, _) = self.db.head().await?;
        if head == 0 {
            self.db.set_head(self.config.start_block)?;
        } else if self.config.start_block != 0 {
            warn!(target: LOG_TARGET, "Start block ignored, stored head exists and will be used instead.");
        }

        let mut backoff_delay = Duration::from_secs(1);
        let max_backoff_delay = Duration::from_secs(60);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let mut erroring_out = false;
        loop {
            let (head, last_pending_block_world_tx, last_pending_block_tx) = self.db.head().await?;
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break Ok(());
                }
                res = self.fetch_data(head, last_pending_block_world_tx, last_pending_block_tx) => {
                    match res {
                        Ok(fetch_result) => {
                            let instant = Instant::now();
                            if erroring_out {
                                erroring_out = false;
                                backoff_delay = Duration::from_secs(1);
                                info!(target: LOG_TARGET, "Syncing reestablished.");
                            }

                            match self.process(fetch_result).await {
                                Ok(()) => self.db.execute()?,
                                Err(e) => {
                                    error!(target: LOG_TARGET, error = %e, "Processing fetched data.");
                                    erroring_out = true;
                                    sleep(backoff_delay).await;
                                    if backoff_delay < max_backoff_delay {
                                        backoff_delay *= 2;
                                    }
                                }
                            }
                            debug!(target: LOG_TARGET, duration = ?instant.elapsed(), "Processed fetched data.");
                        }
                        Err(e) => {
                            erroring_out = true;
                            error!(target: LOG_TARGET, error = %e, "Fetching data.");
                            sleep(backoff_delay).await;
                            if backoff_delay < max_backoff_delay {
                                backoff_delay *= 2;
                            }
                        }
                    };
                    sleep(self.config.polling_interval).await;
                }
            }
        }
    }

    pub async fn fetch_data(
        &mut self,
        from: u64,
        last_pending_block_world_tx: Option<Felt>,
        last_pending_block_tx: Option<Felt>,
    ) -> Result<FetchDataResult> {
        let instant = Instant::now();
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        let result = if from < latest_block_number {
            let from = if from == 0 { from } else { from + 1 };
            let data =
                self.fetch_range(from, latest_block_number, last_pending_block_world_tx).await?;
            debug!(target: LOG_TARGET, duration = ?instant.elapsed(), from = %from, to = %latest_block_number, "Fetched data for range.");
            FetchDataResult::Range(data)
        } else if self.config.index_pending {
            let data = self.fetch_pending(latest_block_number + 1, last_pending_block_tx).await?;
            debug!(target: LOG_TARGET, duration = ?instant.elapsed(), latest_block_number = %latest_block_number, "Fetched pending data.");
            if let Some(data) = data {
                FetchDataResult::Pending(data)
            } else {
                FetchDataResult::None
            }
        } else {
            FetchDataResult::None
        };

        Ok(result)
    }

    pub async fn fetch_range(
        &mut self,
        from: u64,
        to: u64,
        last_pending_block_world_tx: Option<Felt>,
    ) -> Result<FetchRangeResult> {
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
            debug!(target: LOG_TARGET, "Fetching events page with continuation token: {}", &token);
            events_pages.push(get_events(Some(token.clone())).await?);
        }

        debug!(target: LOG_TARGET, "Total events pages fetched: {}", &events_pages.len());
        // Transactions & blocks to process
        let mut last_block = 0_u64;
        let mut blocks = BTreeMap::new();

        // Flatten events pages and events according to the pending block cursor
        // to array of (block_number, transaction_hash)
        let mut last_pending_block_world_tx_cursor = last_pending_block_world_tx;
        let mut transactions = LinkedHashMap::new();
        for events_page in events_pages {
            debug!("Processing events page with events: {}", &events_page.events.len());
            for event in events_page.events {
                let block_number = match event.block_number {
                    Some(block_number) => block_number,
                    // If the block number is not present, try to fetch it from the transaction
                    // receipt Should not/rarely happen. Thus the additional
                    // fetch is acceptable.
                    None => {
                        let TransactionReceiptWithBlockInfo { receipt, block } =
                            self.provider.get_transaction_receipt(event.transaction_hash).await?;

                        match receipt {
                            TransactionReceipt::Invoke(_) | TransactionReceipt::L1Handler(_) => {
                                if let ReceiptBlock::Block { block_number, .. } = block {
                                    block_number
                                } else {
                                    // If the block is pending, we assume the block number is the
                                    // latest + 1
                                    to + 1
                                }
                            }

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
                if let Some(tx) = last_pending_block_world_tx_cursor {
                    if event.transaction_hash != tx {
                        continue;
                    }

                    last_pending_block_world_tx_cursor = None;
                }

                // Skip the latest pending block transaction events
                // * as we might have multiple events for the same transaction
                if let Some(tx) = last_pending_block_world_tx {
                    if event.transaction_hash == tx {
                        continue;
                    }
                }

                transactions
                    .entry((block_number, event.transaction_hash))
                    .or_insert(vec![])
                    .push(event);
            }
        }

        debug!("Transactions: {}", &transactions.len());
        debug!("Blocks: {}", &blocks.len());

        Ok(FetchRangeResult { transactions, blocks, latest_block_number: to })
    }

    async fn fetch_pending(
        &self,
        block_number: u64,
        last_pending_block_tx: Option<Felt>,
    ) -> Result<Option<FetchPendingResult>> {
        let block = if let MaybePendingBlockWithReceipts::PendingBlock(pending) =
            self.provider.get_block_with_receipts(BlockId::Tag(BlockTag::Pending)).await?
        {
            pending
        } else {
            // TODO: change this to unreachable once katana is updated to return PendingBlockWithTxs
            // when BlockTag is Pending unreachable!("We requested pending block, so it
            // must be pending");
            return Ok(None);
        };

        Ok(Some(FetchPendingResult {
            pending_block: Box::new(block),
            block_number,
            last_pending_block_tx,
        }))
    }

    pub async fn process(&mut self, fetch_result: FetchDataResult) -> Result<()> {
        match fetch_result {
            FetchDataResult::Range(data) => {
                self.process_range(data).await?;
            }
            FetchDataResult::Pending(data) => {
                self.process_pending(data).await?;
            }
            FetchDataResult::None => {}
        }

        Ok(())
    }

    pub async fn process_pending(&mut self, data: FetchPendingResult) -> Result<()> {
        // Skip transactions that have been processed already
        // Our cursor is the last processed transaction

        let mut last_pending_block_tx_cursor = data.last_pending_block_tx;
        let mut last_pending_block_tx = data.last_pending_block_tx;
        let mut last_pending_block_world_tx = None;

        let timestamp = data.pending_block.timestamp;

        for t in data.pending_block.transactions {
            let transaction_hash = t.transaction.transaction_hash();
            if let Some(tx) = last_pending_block_tx_cursor {
                if transaction_hash != &tx {
                    continue;
                }

                last_pending_block_tx_cursor = None;
                continue;
            }

            match self.process_transaction_with_receipt(&t, data.block_number, timestamp).await {
                Err(e) => {
                    match e.to_string().as_str() {
                        "TransactionHashNotFound" => {
                            // We failed to fetch the transaction, which is because
                            // the transaction might not have been processed fast enough by the
                            // provider. So we can fail silently and try
                            // again in the next iteration.
                            warn!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction_hash), "Retrieving pending transaction receipt.");
                            self.db.set_head(data.block_number - 1)?;
                            if let Some(tx) = last_pending_block_tx {
                                self.db.set_last_pending_block_tx(Some(tx))?;
                            }

                            if let Some(tx) = last_pending_block_world_tx {
                                self.db.set_last_pending_block_world_tx(Some(tx))?;
                            }
                            return Ok(());
                        }
                        _ => {
                            error!(target: LOG_TARGET, error = %e, transaction_hash = %format!("{:#x}", transaction_hash), "Processing pending transaction.");
                            return Err(e);
                        }
                    }
                }
                Ok(true) => {
                    last_pending_block_world_tx = Some(*transaction_hash);
                    last_pending_block_tx = Some(*transaction_hash);
                    info!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction_hash), "Processed pending world transaction.");
                }
                Ok(_) => {
                    last_pending_block_tx = Some(*transaction_hash);
                    debug!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction_hash), "Processed pending transaction.")
                }
            }
        }

        // Process parallelized events
        self.process_tasks().await?;

        // Set the head to the last processed pending transaction
        // Head block number should still be latest block number
        self.db.set_head(data.block_number - 1)?;

        if let Some(tx) = last_pending_block_tx {
            self.db.set_last_pending_block_tx(Some(tx))?;
        }

        if let Some(tx) = last_pending_block_world_tx {
            self.db.set_last_pending_block_world_tx(Some(tx))?;
        }

        Ok(())
    }

    pub async fn process_range(&mut self, data: FetchRangeResult) -> Result<()> {
        // Process all transactions
        let mut last_block = 0;
        for ((block_number, transaction_hash), events) in data.transactions {
            debug!("Processing transaction hash: {:#x}", transaction_hash);
            // Process transaction
            let transaction = if self.config.flags.contains(IndexingFlags::TRANSACTIONS) {
                Some(self.provider.get_transaction_by_hash(transaction_hash).await?)
            } else {
                None
            };

            self.process_transaction_with_events(
                transaction_hash,
                events.as_slice(),
                block_number,
                data.blocks[&block_number],
                transaction,
            )
            .await?;

            // Process block
            if block_number > last_block {
                if let Some(ref block_tx) = self.block_tx {
                    block_tx.send(block_number).await?;
                }

                self.process_block(block_number, data.blocks[&block_number]).await?;
                last_block = block_number;
            }
        }

        // Process parallelized events
        self.process_tasks().await?;

        self.db.set_head(data.latest_block_number)?;
        self.db.set_last_pending_block_world_tx(None)?;
        self.db.set_last_pending_block_tx(None)?;

        Ok(())
    }

    async fn process_tasks(&mut self) -> Result<()> {
        // We use a semaphore to limit the number of concurrent tasks
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_tasks));

        // Run all tasks concurrently
        let mut handles = Vec::new();
        for (task_id, events) in self.tasks.drain() {
            let db = self.db.clone();
            let world = self.world.clone();
            let processors = self.processors.clone();
            let semaphore = semaphore.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                let mut local_db = db.clone();
                for ParallelizedEvent { event_id, event, block_number, block_timestamp } in events {
                    if let Some(processor) = processors.event.get(&event.keys[0]) {
                        debug!(target: LOG_TARGET, event_name = processor.event_key(), task_id = %task_id, "Processing parallelized event.");

                        if let Err(e) = processor
                            .process(&world, &mut local_db, block_number, block_timestamp, &event_id, &event)
                            .await
                        {
                            error!(target: LOG_TARGET, event_name = processor.event_key(), error = %e, task_id = %task_id, "Processing parallelized event.");
                        }
                    }
                }
                Ok::<_, anyhow::Error>(local_db)
            }));
        }

        // Join all tasks
        try_join_all(handles).await?;

        Ok(())
    }

    async fn get_block_timestamp(&self, block_number: u64) -> Result<u64> {
        match self.provider.get_block_with_tx_hashes(BlockId::Number(block_number)).await? {
            MaybePendingBlockWithTxHashes::Block(block) => Ok(block.timestamp),
            MaybePendingBlockWithTxHashes::PendingBlock(block) => Ok(block.timestamp),
        }
    }

    async fn process_transaction_with_events(
        &mut self,
        transaction_hash: Felt,
        events: &[EmittedEvent],
        block_number: u64,
        block_timestamp: u64,
        transaction: Option<Transaction>,
    ) -> Result<()> {
        for (event_idx, event) in events.iter().enumerate() {
            let event_id =
                format!("{:#064x}:{:#x}:{:#04x}", block_number, transaction_hash, event_idx);

            let event = Event {
                from_address: event.from_address,
                keys: event.keys.clone(),
                data: event.data.clone(),
            };
            Self::process_event(
                self,
                block_number,
                block_timestamp,
                &event_id,
                &event,
                transaction_hash,
            )
            .await?;
        }

        if let Some(ref transaction) = transaction {
            Self::process_transaction(
                self,
                block_number,
                block_timestamp,
                transaction_hash,
                transaction,
            )
            .await?;
        }

        Ok(())
    }

    // Process a transaction and events from its receipt.
    // Returns whether the transaction has a world event.
    async fn process_transaction_with_receipt(
        &mut self,
        transaction_with_receipt: &TransactionWithReceipt,
        block_number: u64,
        block_timestamp: u64,
    ) -> Result<bool> {
        let transaction_hash = transaction_with_receipt.transaction.transaction_hash();
        let events = match &transaction_with_receipt.receipt {
            TransactionReceipt::Invoke(receipt) => Some(&receipt.events),
            TransactionReceipt::L1Handler(receipt) => Some(&receipt.events),
            _ => None,
        };

        let mut world_event = false;
        if let Some(events) = events {
            for (event_idx, event) in events.iter().enumerate() {
                if event.from_address != self.world.address {
                    continue;
                }

                world_event = true;
                let event_id =
                    format!("{:#064x}:{:#x}:{:#04x}", block_number, *transaction_hash, event_idx);

                Self::process_event(
                    self,
                    block_number,
                    block_timestamp,
                    &event_id,
                    event,
                    *transaction_hash,
                )
                .await?;
            }

            if world_event && self.config.flags.contains(IndexingFlags::TRANSACTIONS) {
                Self::process_transaction(
                    self,
                    block_number,
                    block_timestamp,
                    *transaction_hash,
                    &transaction_with_receipt.transaction,
                )
                .await?;
            }
        }

        Ok(world_event)
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
        event_id: &str,
        event: &Event,
        transaction_hash: Felt,
    ) -> Result<()> {
        if self.config.flags.contains(IndexingFlags::RAW_EVENTS) {
            self.db.store_event(event_id, event, transaction_hash, block_timestamp)?;
        }

        let event_key = event.keys[0];

        let Some(processor) = self.processors.event.get(&event_key) else {
            // if we dont have a processor for this event, we try the catch all processor
            if self.processors.catch_all_event.validate(event) {
                if let Err(e) = self
                    .processors
                    .catch_all_event
                    .process(
                        &self.world,
                        &mut self.db,
                        block_number,
                        block_timestamp,
                        event_id,
                        event,
                    )
                    .await
                {
                    error!(target: LOG_TARGET, error = %e, "Processing catch all event processor.");
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

            return Ok(());
        };

        let task_identifier = match processor.event_key().as_str() {
            "StoreSetRecord" | "StoreUpdateRecord" | "StoreUpdateMember" | "StoreDelRecord" => {
                let mut hasher = DefaultHasher::new();
                event.data[0].hash(&mut hasher);
                event.data[1].hash(&mut hasher);
                hasher.finish()
            }
            _ => 0,
        };

        // if we have a task identifier, we queue the event to be parallelized
        if task_identifier != 0 {
            self.tasks.entry(task_identifier).or_default().push(ParallelizedEvent {
                event_id: event_id.to_string(),
                event: event.clone(),
                block_number,
                block_timestamp,
            });
        } else {
            // if we dont have a task identifier, we process the event immediately
            if let Err(e) = processor
                .process(&self.world, &mut self.db, block_number, block_timestamp, event_id, event)
                .await
            {
                error!(target: LOG_TARGET, event_name = processor.event_key(), error = %e, "Processing event.");
            }
        }

        Ok(())
    }
}
