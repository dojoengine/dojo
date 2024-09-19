use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bitflags::bitflags;
use dojo_world::contracts::world::WorldContractReader;
use futures_util::future::join_all;
use hashlink::LinkedHashMap;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, Event, EventFilter, Felt, MaybePendingBlockWithReceipts,
    MaybePendingBlockWithTxHashes, PendingBlockWithReceipts, ReceiptBlock, TransactionReceipt,
    TransactionReceiptWithBlockInfo, TransactionWithReceipt,
};
use starknet::providers::Provider;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Sender as BoundedSender;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

use crate::processors::event_message::EventMessageProcessor;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::sql::{Cursors, Sql};
use crate::types::ErcContract;

#[allow(missing_debug_implementations)]
pub struct Processors<P: Provider + Send + Sync + std::fmt::Debug + 'static> {
    pub block: Vec<Box<dyn BlockProcessor<P>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<P>>>,
    pub event: HashMap<Felt, Vec<Box<dyn EventProcessor<P>>>>,
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
    // NOTE: LinkedList might contains blocks in different order
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
    provider: Arc<P>,
    processors: Arc<Processors<P>>,
    config: EngineConfig,
    shutdown_tx: Sender<()>,
    block_tx: Option<BoundedSender<u64>>,
    // ERC tokens to index
    tokens: HashMap<Felt, ErcContract>,
    tasks: HashMap<u64, Vec<ParallelizedEvent>>,
}

struct UnprocessedEvent {
    keys: Vec<String>,
    data: Vec<String>,
}

impl<P: Provider + Send + Sync + std::fmt::Debug + 'static> Engine<P> {
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
            world: Arc::new(world),
            db,
            provider: Arc::new(provider),
            processors: Arc::new(processors),
            config,
            shutdown_tx,
            block_tx,
            tokens,
            tasks: HashMap::new(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        // use the start block provided by user if head is 0
        let (head, _, _) = self.db.head(self.world.address).await?;
        if head == 0 {
            self.db.set_head(self.world.address, self.config.start_block);
        } else if self.config.start_block != 0 {
            warn!(target: LOG_TARGET, "Start block ignored, stored head exists and will be used instead.");
        }

        let mut backoff_delay = Duration::from_secs(1);
        let max_backoff_delay = Duration::from_secs(60);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let mut erroring_out = false;
        loop {
            let cursors = self.db.cursors().await?;
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break Ok(());
                }
                res = self.fetch_data(&cursors) => {
                    match res {
                        Ok(fetch_result) => {
                            if erroring_out {
                                erroring_out = false;
                                backoff_delay = Duration::from_secs(1);
                                info!(target: LOG_TARGET, "Syncing reestablished.");
                            }

                            match self.process(fetch_result).await {
                                Ok(()) => {}
                                Err(e) => {
                                    error!(target: LOG_TARGET, error = %e, "Processing fetched data.");
                                    erroring_out = true;
                                    sleep(backoff_delay).await;
                                    if backoff_delay < max_backoff_delay {
                                        backoff_delay *= 2;
                                    }
                                }
                            }
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

    pub async fn fetch_data(&mut self, cursors: &Cursors) -> Result<FetchDataResult> {
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;
        let from = cursors.head.unwrap_or(0);

        let result = if from < latest_block_number {
            let from = if from == 0 { from } else { from + 1 };
            debug!(target: LOG_TARGET, from = %from, to = %latest_block_number, "Fetching data for range.");
            let data = self.fetch_range(from, latest_block_number, &cursors.cursor_map).await?;
            FetchDataResult::Range(data)
        } else if self.config.index_pending {
            let data =
                self.fetch_pending(latest_block_number + 1, cursors.last_pending_block_tx).await?;
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
        cursor_map: &HashMap<Felt, Felt>,
    ) -> Result<FetchRangeResult> {
        // Process all blocks from current to latest.
        let world_events_filter = EventFilter {
            from_block: Some(BlockId::Number(from)),
            to_block: Some(BlockId::Number(to)),
            address: Some(self.world.address),
            keys: None,
        };

        let mut fetch_all_events_tasks = vec![];
        let world_events_pages =
            get_all_events(&self.provider, world_events_filter, self.config.events_chunk_size);

        fetch_all_events_tasks.push(world_events_pages);

        for token in self.tokens.iter() {
            let events_filter = EventFilter {
                from_block: Some(BlockId::Number(from)),
                to_block: Some(BlockId::Number(to)),
                address: Some(*token.0),
                keys: None,
            };
            let token_events_pages =
                get_all_events(&self.provider, events_filter, self.config.events_chunk_size);

            fetch_all_events_tasks.push(token_events_pages);
        }

        let task_result = join_all(fetch_all_events_tasks).await;

        let mut events = vec![];

        for result in task_result {
            let result = result?;
            let contract_address =
                result.0.expect("EventFilters that we use always have an address");
            let events_pages = result.1;
            let last_contract_tx = cursor_map.get(&contract_address).cloned();
            let mut last_contract_tx_tmp = last_contract_tx;
            debug!(target: LOG_TARGET, "Total events pages fetched for contract ({:#x}): {}", &contract_address, &events_pages.len());

            for events_page in events_pages {
                debug!("Processing events page with events: {}", &events_page.events.len());
                for event in events_page.events {
                    // Then we skip all transactions until we reach the last pending processed
                    // transaction (if any)
                    if let Some(last_contract_tx) = last_contract_tx_tmp {
                        if event.transaction_hash != last_contract_tx {
                            continue;
                        }

                        last_contract_tx_tmp = None;
                    }

                    // Skip the latest pending block transaction events
                    // * as we might have multiple events for the same transaction
                    if let Some(last_contract_tx) = last_contract_tx {
                        if event.transaction_hash == last_contract_tx {
                            continue;
                        }
                    }

                    events.push(event);
                }
            }
        }

        // Transactions & blocks to process
        let mut blocks = BTreeMap::new();

        // Flatten events pages and events according to the pending block cursor
        // to array of (block_number, transaction_hash)
        let mut transactions = LinkedHashMap::new();

        let mut block_set = HashSet::new();
        for event in events {
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

            block_set.insert(block_number);

            transactions
                .entry((block_number, event.transaction_hash))
                .or_insert(vec![])
                .push(event);
        }

        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_tasks));
        let mut set: JoinSet<Result<(u64, u64), anyhow::Error>> = JoinSet::new();

        for block_number in block_set {
            let semaphore = semaphore.clone();
            let provider = self.provider.clone();
            set.spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                debug!("Fetching block timestamp for block number: {}", block_number);
                let block_timestamp = get_block_timestamp(&provider, block_number).await?;
                Ok((block_number, block_timestamp))
            });
        }

        while let Some(result) = set.join_next().await {
            let (block_number, block_timestamp) = result??;
            blocks.insert(block_number, block_timestamp);
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

        let timestamp = data.pending_block.timestamp;

        let mut cursor_map = HashMap::new();
        for t in data.pending_block.transactions {
            let transaction_hash = t.transaction.transaction_hash();
            if let Some(tx) = last_pending_block_tx_cursor {
                if transaction_hash != &tx {
                    continue;
                }

                last_pending_block_tx_cursor = None;
                continue;
            }

            match self
                .process_transaction_with_receipt(&t, data.block_number, timestamp, &mut cursor_map)
                .await
            {
                Err(e) => {
                    match e.to_string().as_str() {
                        // TODO: remove this we now fetch the pending block with receipts so this
                        // error is no longer relevant
                        "TransactionHashNotFound" => {
                            // We failed to fetch the transaction, which is because
                            // the transaction might not have been processed fast enough by the
                            // provider. So we can fail silently and try
                            // again in the next iteration.
                            warn!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction_hash), "Retrieving pending transaction receipt.");
                            self.db.set_head(self.world.address, data.block_number - 1);
                            if let Some(tx) = last_pending_block_tx {
                                self.db.set_last_pending_block_tx(Some(tx));
                            }

                            self.db.execute().await?;
                            return Ok(());
                        }
                        _ => {
                            error!(target: LOG_TARGET, error = %e, transaction_hash = %format!("{:#x}", transaction_hash), "Processing pending transaction.");
                            return Err(e);
                        }
                    }
                }
                Ok(_) => {
                    last_pending_block_tx = Some(*transaction_hash);
                    debug!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction_hash), "Processed pending transaction.")
                }
            }
        }

        // Process parallelized events
        self.process_tasks().await?;

        // Head block number should still be latest block number
        self.db.update_cursors(data.block_number - 1, last_pending_block_tx, cursor_map);

        self.db.execute().await?;

        Ok(())
    }

    pub async fn process_range(&mut self, data: FetchRangeResult) -> Result<()> {
        // Process all transactions
        let mut processed_blocks = HashSet::new();
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
            if !processed_blocks.contains(&block_number) {
                if let Some(ref block_tx) = self.block_tx {
                    block_tx.send(block_number).await?;
                }

                self.process_block(block_number, data.blocks[&block_number]).await?;
                processed_blocks.insert(block_number);
            }

            if self.db.query_queue.queue.len() >= QUERY_QUEUE_BATCH_SIZE {
                self.db.execute().await?;
            }
        }

        // Process parallelized events
        self.process_tasks().await?;

        self.db.reset_cursors(data.latest_block_number);

        self.db.execute().await?;

        Ok(())
    }

    async fn process_tasks(&mut self) -> Result<()> {
        // We use a semaphore to limit the number of concurrent tasks
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_tasks));

        // Run all tasks concurrently
        let mut set = JoinSet::new();
        for (task_id, events) in self.tasks.drain() {
            let db = self.db.clone();
            let world = self.world.clone();
            let processors = self.processors.clone();
            let semaphore = semaphore.clone();

            set.spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                let mut local_db = db.clone();
                for ParallelizedEvent { event_id, event, block_number, block_timestamp } in events {
                    if let Some(event_processors) = processors.event.get(&event.keys[0]) {
                        for processor in event_processors.iter() {
                            debug!(target: LOG_TARGET, event_name = processor.event_key(), task_id = %task_id, "Processing parallelized event.");

                            if let Err(e) = processor
                                .process(&world, &mut local_db, block_number, block_timestamp, &event_id, &event)
                                .await
                            {
                                error!(target: LOG_TARGET, event_name = processor.event_key(), error = %e, task_id = %task_id, "Processing parallelized event.");
                            }
                        }
                    }
                }
                Ok::<_, anyhow::Error>(local_db)
            });
        }

        // Join all tasks
        while let Some(result) = set.join_next().await {
            let local_db = result??;
            self.db.merge(local_db)?;
        }

        Ok(())
    }

    async fn process_transaction_with_events(
        &mut self,
        transaction_hash: Felt,
        events: &[EmittedEvent],
        block_number: u64,
        block_timestamp: u64,
        transaction: Option<Transaction>,
    ) -> Result<()> {
        // Contract -> Cursor
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
        cursor_map: &mut HashMap<Felt, Felt>,
    ) -> Result<()> {
        let transaction_hash = transaction_with_receipt.transaction.transaction_hash();
        let events = match &transaction_with_receipt.receipt {
            TransactionReceipt::Invoke(receipt) => Some(&receipt.events),
            TransactionReceipt::L1Handler(receipt) => Some(&receipt.events),
            _ => None,
        };

        if let Some(events) = events {
            for (event_idx, event) in events.iter().enumerate() {
                if event.from_address != self.world.address
                    && !self.tokens.contains_key(&event.from_address)
                {
                    continue;
                }

                cursor_map.insert(event.from_address, *transaction_hash);
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
            self.db.store_event(event_id, event, transaction_hash, block_timestamp);
        }

        let event_key = event.keys[0];

        let Some(processors) = self.processors.event.get(&event_key) else {
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

        // For now we only have 1 processor for store* events
        let task_identifier = if processors.len() == 1 {
            match processors[0].event_key().as_str() {
                "StoreSetRecord" | "StoreUpdateRecord" | "StoreUpdateMember" | "StoreDelRecord" => {
                    let mut hasher = DefaultHasher::new();
                    event.data[0].hash(&mut hasher);
                    event.data[1].hash(&mut hasher);
                    hasher.finish()
                }
                _ => 0,
            }
        } else {
            0
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
            for processor in processors.iter() {
                if !processor.validate(event) {
                    continue;
                }

                if let Err(e) = processor
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
                    error!(target: LOG_TARGET, event_name = processor.event_key(), error = ?e, "Processing event.");
                }
            }
        }

        Ok(())
    }
}

async fn get_all_events<P>(
    provider: &P,
    events_filter: EventFilter,
    events_chunk_size: u64,
) -> Result<(Option<Felt>, Vec<EventsPage>)>
where
    P: Provider + Sync,
{
    let mut events_pages = Vec::new();
    let mut continuation_token = None;

    loop {
        debug!(
            "Fetching events page with continuation token: {:?}, for contract: {:?}",
            continuation_token, events_filter.address
        );
        let events_page = provider
            .get_events(events_filter.clone(), continuation_token.clone(), events_chunk_size)
            .await?;

        continuation_token = events_page.continuation_token.clone();
        events_pages.push(events_page);

        if continuation_token.is_none() {
            break;
        }
    }

    Ok((events_filter.address, events_pages))
}

async fn get_block_timestamp<P>(provider: &P, block_number: u64) -> Result<u64>
where
    P: Provider + Sync,
{
    match provider.get_block_with_tx_hashes(BlockId::Number(block_number)).await? {
        MaybePendingBlockWithTxHashes::Block(block) => Ok(block.timestamp),
        MaybePendingBlockWithTxHashes::PendingBlock(block) => Ok(block.timestamp),
    }
}
