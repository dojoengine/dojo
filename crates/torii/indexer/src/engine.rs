use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_recursion::async_recursion;
use bitflags::bitflags;
use dojo_utils::provider as provider_utils;
use dojo_world::contracts::world::WorldContractReader;
use hashlink::LinkedHashMap;
use starknet::core::types::requests::{GetBlockWithTxHashesRequest, GetEventsRequest};
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BlockTag, EmittedEvent, Event, EventFilter, EventFilterWithPage,
    MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, PendingBlockWithReceipts,
    ResultPageRequest, Transaction, TransactionReceipt, TransactionWithReceipt,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::{Provider, ProviderRequestData, ProviderResponseData};
use starknet_crypto::Felt;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Sender as BoundedSender;
use tokio::time::{sleep, Instant};
use torii_sqlite::cache::ContractClassCache;
use torii_sqlite::types::{Contract, ContractType};
use torii_sqlite::{Cursors, Sql};
use tracing::{debug, error, info, trace, warn};

use crate::constants::LOG_TARGET;
use crate::processors::controller::ControllerProcessor;
use crate::processors::erc1155_transfer_batch::Erc1155TransferBatchProcessor;
use crate::processors::erc1155_transfer_single::Erc1155TransferSingleProcessor;
use crate::processors::erc20_legacy_transfer::Erc20LegacyTransferProcessor;
use crate::processors::erc20_transfer::Erc20TransferProcessor;
use crate::processors::erc4906_batch_metadata_update::Erc4906BatchMetadataUpdateProcessor;
use crate::processors::erc4906_metadata_update::Erc4906MetadataUpdateProcessor;
use crate::processors::erc721_legacy_transfer::Erc721LegacyTransferProcessor;
use crate::processors::erc721_transfer::Erc721TransferProcessor;
use crate::processors::event_message::EventMessageProcessor;
use crate::processors::metadata_update::MetadataUpdateProcessor;
use crate::processors::raw_event::RawEventProcessor;
use crate::processors::register_event::RegisterEventProcessor;
use crate::processors::register_model::RegisterModelProcessor;
use crate::processors::store_del_record::StoreDelRecordProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::processors::store_transaction::StoreTransactionProcessor;
use crate::processors::store_update_member::StoreUpdateMemberProcessor;
use crate::processors::store_update_record::StoreUpdateRecordProcessor;
use crate::processors::upgrade_event::UpgradeEventProcessor;
use crate::processors::upgrade_model::UpgradeModelProcessor;
use crate::processors::{
    BlockProcessor, EventProcessor, EventProcessorConfig, TransactionProcessor,
};
use crate::task_manager::{self, ParallelizedEvent, TaskManager};

type EventProcessorMap<P> = HashMap<Felt, Vec<Box<dyn EventProcessor<P>>>>;

#[allow(missing_debug_implementations)]
pub struct Processors<P: Provider + Send + Sync + std::fmt::Debug + 'static> {
    pub block: Vec<Box<dyn BlockProcessor<P>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<P>>>,
    pub catch_all_event: Box<dyn EventProcessor<P>>,
    pub event_processors: HashMap<ContractType, EventProcessorMap<P>>,
}

impl<P: Provider + Send + Sync + std::fmt::Debug + 'static> Default for Processors<P> {
    fn default() -> Self {
        Self {
            block: vec![],
            transaction: vec![Box::new(StoreTransactionProcessor)],
            // We shouldn't have a catch all for now since the world doesn't forward raw events
            // anymore.
            catch_all_event: Box::new(RawEventProcessor) as Box<dyn EventProcessor<P>>,
            event_processors: Self::initialize_event_processors(),
        }
    }
}

impl<P: Provider + Send + Sync + std::fmt::Debug + 'static> Processors<P> {
    pub fn initialize_event_processors() -> HashMap<ContractType, EventProcessorMap<P>> {
        let mut event_processors_map = HashMap::<ContractType, EventProcessorMap<P>>::new();

        let event_processors = vec![
            (
                ContractType::WORLD,
                vec![
                    Box::new(RegisterModelProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(RegisterEventProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(UpgradeModelProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(UpgradeEventProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(StoreSetRecordProcessor),
                    Box::new(StoreDelRecordProcessor),
                    Box::new(StoreUpdateRecordProcessor),
                    Box::new(StoreUpdateMemberProcessor),
                    Box::new(MetadataUpdateProcessor),
                    Box::new(EventMessageProcessor),
                ],
            ),
            (
                ContractType::ERC20,
                vec![
                    Box::new(Erc20TransferProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(Erc20LegacyTransferProcessor) as Box<dyn EventProcessor<P>>,
                ],
            ),
            (
                ContractType::ERC721,
                vec![
                    Box::new(Erc721TransferProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(Erc721LegacyTransferProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(Erc4906MetadataUpdateProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(Erc4906BatchMetadataUpdateProcessor) as Box<dyn EventProcessor<P>>,
                ],
            ),
            (
                ContractType::ERC1155,
                vec![
                    Box::new(Erc1155TransferBatchProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(Erc1155TransferSingleProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(Erc4906MetadataUpdateProcessor) as Box<dyn EventProcessor<P>>,
                    Box::new(Erc4906BatchMetadataUpdateProcessor) as Box<dyn EventProcessor<P>>,
                ],
            ),
            (ContractType::UDC, vec![Box::new(ControllerProcessor) as Box<dyn EventProcessor<P>>]),
        ];

        for (contract_type, processors) in event_processors {
            for processor in processors {
                let key = get_selector_from_name(processor.event_key().as_str())
                    .expect("Event key is ASCII so this should never fail");
                // event_processors_map.entry(contract_type).or_default().insert(key, processor);
                event_processors_map
                    .entry(contract_type)
                    .or_default()
                    .entry(key)
                    .or_default()
                    .push(processor);
            }
        }

        event_processors_map
    }

    pub fn get_event_processor(
        &self,
        contract_type: ContractType,
    ) -> &HashMap<Felt, Vec<Box<dyn EventProcessor<P>>>> {
        self.event_processors.get(&contract_type).unwrap()
    }
}

bitflags! {
    #[derive(Debug, Clone)]
    pub struct IndexingFlags: u32 {
        const TRANSACTIONS = 0b00000001;
        const RAW_EVENTS = 0b00000010;
        const PENDING_BLOCKS = 0b00000100;
    }
}

#[derive(Debug)]
pub struct EngineConfig {
    pub polling_interval: Duration,
    pub blocks_chunk_size: u64,
    pub events_chunk_size: u64,
    pub max_concurrent_tasks: usize,
    pub flags: IndexingFlags,
    pub event_processor_config: EventProcessorConfig,
    pub world_block: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_millis(500),
            blocks_chunk_size: 10240,
            events_chunk_size: 1024,
            max_concurrent_tasks: 100,
            flags: IndexingFlags::empty(),
            event_processor_config: EventProcessorConfig::default(),
            world_block: 0,
        }
    }
}

#[derive(Debug)]
pub enum FetchDataResult {
    Range(FetchRangeResult),
    Pending(FetchPendingResult),
    None,
}

impl FetchDataResult {
    pub fn block_id(&self) -> Option<BlockId> {
        match self {
            FetchDataResult::Range(range) => Some(BlockId::Number(range.latest_block_number)),
            FetchDataResult::Pending(_pending) => Some(BlockId::Tag(BlockTag::Pending)),
            // we dont require block_id when result is none, we return None
            FetchDataResult::None => None,
        }
    }
}

#[derive(Debug)]
pub struct FetchRangeResult {
    // (block_number, transaction_hash) -> events
    // NOTE: LinkedList might contains blocks in different order
    pub transactions: BTreeMap<u64, LinkedHashMap<Felt, Vec<EmittedEvent>>>,
    pub blocks: BTreeMap<u64, u64>,
    pub latest_block_number: u64,
}

#[derive(Debug)]
pub struct FetchPendingResult {
    pub pending_block: Box<PendingBlockWithReceipts>,
    pub last_pending_block_tx: Option<Felt>,
    pub block_number: u64,
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
    task_manager: TaskManager<P>,
    contracts: Arc<HashMap<Felt, ContractType>>,
    contract_class_cache: Arc<ContractClassCache<P>>,
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
        contracts: &[Contract],
    ) -> Self {
        let contracts = Arc::new(
            contracts.iter().map(|contract| (contract.address, contract.r#type)).collect(),
        );
        let world = Arc::new(world);
        let processors = Arc::new(processors);
        let max_concurrent_tasks = config.max_concurrent_tasks;
        let event_processor_config = config.event_processor_config.clone();
        let provider = Arc::new(provider);

        Self {
            world: world.clone(),
            db: db.clone(),
            provider: provider.clone(),
            processors: processors.clone(),
            config,
            shutdown_tx,
            block_tx,
            contracts,
            task_manager: TaskManager::new(
                db,
                world,
                processors,
                max_concurrent_tasks,
                event_processor_config,
            ),
            contract_class_cache: Arc::new(ContractClassCache::new(provider)),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if let Err(e) = provider_utils::health_check_provider(self.provider.clone()).await {
            error!(target: LOG_TARGET,"Provider health check failed during engine start");
            return Err(e);
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
                            let instant = Instant::now();
                            if erroring_out {
                                erroring_out = false;
                                backoff_delay = Duration::from_secs(1);
                                info!(target: LOG_TARGET, "Syncing reestablished.");
                            }

                            let block_id = fetch_result.block_id();
                            match self.process(fetch_result).await {
                                Ok(_) => {
                                    // Its only `None` when `FetchDataResult::None` in which case
                                    // we don't need to flush or apply cache diff
                                    if let Some(block_id) = block_id {
                                        self.db.flush().await?;
                                        self.db.apply_cache_diff().await?;
                                        self.db.execute().await?;
                                        debug!(target: LOG_TARGET, block_number = ?block_id, "Flushed and applied cache diff.");
                                    }
                                },
                                Err(e) => {
                                    error!(target: LOG_TARGET, error = %e, "Processing fetched data.");
                                    erroring_out = true;
                                    // incase of error rollback the transaction
                                    self.db.rollback().await?;
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

    // TODO: since we now process blocks in chunks we can parallelize the fetching of data
    pub async fn fetch_data(&mut self, cursors: &Cursors) -> Result<FetchDataResult> {
        let latest_block = self.provider.block_hash_and_number().await?;

        let from = cursors.head.unwrap_or(self.config.world_block);
        let total_remaining_blocks = latest_block.block_number - from;
        let blocks_to_process = total_remaining_blocks.min(self.config.blocks_chunk_size);
        let to = (from + blocks_to_process).min(latest_block.block_number);

        let instant = Instant::now();
        let result = if from < latest_block.block_number {
            let from = if from == 0 { from } else { from + 1 };
            let data = self.fetch_range(from, to, &cursors.cursor_map).await?;
            debug!(target: LOG_TARGET, duration = ?instant.elapsed(), from = %from, to = %to, "Fetched data for range.");
            FetchDataResult::Range(data)
        } else if self.config.flags.contains(IndexingFlags::PENDING_BLOCKS) {
            let data =
                self.fetch_pending(latest_block.clone(), cursors.last_pending_block_tx).await?;
            debug!(target: LOG_TARGET, duration = ?instant.elapsed(), latest_block_number = %latest_block.block_number, "Fetched pending data.");
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
        let mut events = vec![];

        // Create initial batch requests for all contracts
        let mut event_requests = Vec::new();
        for (contract_address, _) in self.contracts.iter() {
            let events_filter = EventFilter {
                from_block: Some(BlockId::Number(from)),
                to_block: Some(BlockId::Number(to)),
                address: Some(*contract_address),
                keys: None,
            };

            event_requests.push((
                *contract_address,
                ProviderRequestData::GetEvents(GetEventsRequest {
                    filter: EventFilterWithPage {
                        event_filter: events_filter,
                        result_page_request: ResultPageRequest {
                            continuation_token: None,
                            chunk_size: self.config.events_chunk_size,
                        },
                    },
                }),
            ));
        }

        // Recursively fetch all events using batch requests
        events.extend(self.fetch_events_recursive(event_requests, cursor_map).await?);

        // Process events to get unique blocks and transactions
        let mut blocks = BTreeMap::new();
        let mut transactions = BTreeMap::new();
        let mut block_numbers = HashSet::new();

        for event in events {
            let block_number = match event.block_number {
                Some(block_number) => block_number,
                None => unreachable!("In fetch range all events should have block number"),
            };

            block_numbers.insert(block_number);

            transactions
                .entry(block_number)
                .or_insert(LinkedHashMap::new())
                .entry(event.transaction_hash)
                .or_insert(vec![])
                .push(event);
        }

        // Batch request block timestamps
        let mut timestamp_requests = Vec::new();
        for block_number in &block_numbers {
            timestamp_requests.push(ProviderRequestData::GetBlockWithTxHashes(
                GetBlockWithTxHashesRequest { block_id: BlockId::Number(*block_number) },
            ));
        }

        // Execute timestamp requests in batch
        if !timestamp_requests.is_empty() {
            let timestamp_results = self.provider.batch_requests(timestamp_requests).await?;

            // Process timestamp results
            for (block_number, result) in block_numbers.iter().zip(timestamp_results) {
                match result {
                    ProviderResponseData::GetBlockWithTxHashes(block) => {
                        let timestamp = match block {
                            MaybePendingBlockWithTxHashes::Block(block) => block.timestamp,
                            MaybePendingBlockWithTxHashes::PendingBlock(block) => block.timestamp,
                        };
                        blocks.insert(*block_number, timestamp);
                    }
                    _ => {
                        error!(target: LOG_TARGET, "Unexpected response type from batch timestamp request");
                        return Err(anyhow::anyhow!(
                            "Unexpected response type from batch timestamp request"
                        ));
                    }
                }
            }
        }

        debug!("Transactions: {}", &transactions.len());
        debug!("Blocks: {}", &blocks.len());

        Ok(FetchRangeResult { transactions, blocks, latest_block_number: to })
    }

    #[async_recursion]
    async fn fetch_events_recursive(
        &self,
        requests: Vec<(Felt, ProviderRequestData)>,
        cursor_map: &HashMap<Felt, Felt>,
    ) -> Result<Vec<EmittedEvent>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let mut events = Vec::new();
        let mut next_requests = Vec::new();

        // Extract just the requests without the contract addresses
        let batch_requests: Vec<ProviderRequestData> =
            requests.iter().map(|(_, req)| req.clone()).collect();
        let batch_results = self.provider.batch_requests(batch_requests).await?;

        // Process results and prepare next batch of requests if needed
        for ((contract_address, original_request), result) in
            requests.into_iter().zip(batch_results)
        {
            match result {
                ProviderResponseData::GetEvents(events_page) => {
                    // Process events for this page
                    for event in events_page.events {
                        let last_contract_tx = cursor_map.get(&contract_address).cloned();

                        // Skip if we haven't reached the last processed transaction
                        if let Some(last_tx) = last_contract_tx {
                            if event.transaction_hash == last_tx {
                                continue;
                            }
                        }

                        events.push(event);
                    }

                    // If there's a continuation token, prepare next request
                    if let Some(continuation_token) = events_page.continuation_token {
                        if let ProviderRequestData::GetEvents(mut next_request) = original_request {
                            next_request.filter.result_page_request.continuation_token =
                                Some(continuation_token);
                            next_requests.push((
                                contract_address,
                                ProviderRequestData::GetEvents(next_request),
                            ));
                        }
                    }
                }
                _ => {
                    error!(target: LOG_TARGET, "Unexpected response type from batch events request");
                    return Err(anyhow::anyhow!(
                        "Unexpected response type from batch events request"
                    ));
                }
            }
        }

        // Recursively fetch next batch if there are any continuation tokens
        if !next_requests.is_empty() {
            events.extend(self.fetch_events_recursive(next_requests, cursor_map).await?);
        }

        Ok(events)
    }

    async fn fetch_pending(
        &self,
        block: BlockHashAndNumber,
        last_pending_block_tx: Option<Felt>,
    ) -> Result<Option<FetchPendingResult>> {
        let pending_block = if let MaybePendingBlockWithReceipts::PendingBlock(pending) =
            self.provider.get_block_with_receipts(BlockId::Tag(BlockTag::Pending)).await?
        {
            // if the parent hash is not the hash of the latest block that we fetched, then it means
            // a new block got mined just after we fetched the latest block information
            if block.block_hash != pending.parent_hash {
                return Ok(None);
            }

            pending
        } else {
            // TODO: change this to unreachable once katana is updated to return PendingBlockWithTxs
            // when BlockTag is Pending unreachable!("We requested pending block, so it
            // must be pending");
            return Ok(None);
        };

        Ok(Some(FetchPendingResult {
            pending_block: Box::new(pending_block),
            block_number: block.block_number + 1,
            last_pending_block_tx,
        }))
    }

    pub async fn process(&mut self, fetch_result: FetchDataResult) -> Result<()> {
        match fetch_result {
            FetchDataResult::Range(data) => self.process_range(data).await?,
            FetchDataResult::Pending(data) => self.process_pending(data).await?,
            FetchDataResult::None => {}
        };

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

            if let Err(e) = self
                .process_transaction_with_receipt(&t, data.block_number, timestamp, &mut cursor_map)
                .await
            {
                error!(target: LOG_TARGET, error = %e, transaction_hash = %format!("{:#x}", transaction_hash), "Processing pending transaction.");
                return Err(e);
            }

            last_pending_block_tx = Some(*transaction_hash);
            debug!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction_hash), "Processed pending transaction.");
        }

        // Process parallelized events
        self.task_manager.process_tasks().await?;

        self.db.update_cursors(
            data.block_number - 1,
            timestamp,
            last_pending_block_tx,
            cursor_map,
        )?;

        Ok(())
    }

    pub async fn process_range(&mut self, data: FetchRangeResult) -> Result<()> {
        // Process all transactions
        let mut processed_blocks = HashSet::new();
        let mut cursor_map = HashMap::new();
        for (block_number, transactions) in data.transactions {
            for (transaction_hash, events) in transactions {
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
                    &mut cursor_map,
                )
                .await?;
            }

            // Process block
            if !processed_blocks.contains(&block_number) {
                if let Some(ref block_tx) = self.block_tx {
                    block_tx.send(block_number).await?;
                }

                self.process_block(block_number, data.blocks[&block_number]).await?;
                processed_blocks.insert(block_number);
            }
        }

        // Process parallelized events
        self.task_manager.process_tasks().await?;

        let last_block_timestamp = data
            .blocks
            .get(&data.latest_block_number)
            .copied()
            .unwrap_or(get_block_timestamp(&self.provider, data.latest_block_number).await?);
        self.db.update_cursors(data.latest_block_number, last_block_timestamp, None, cursor_map)?;

        Ok(())
    }

    async fn process_transaction_with_events(
        &mut self,
        transaction_hash: Felt,
        events: &[EmittedEvent],
        block_number: u64,
        block_timestamp: u64,
        transaction: Option<Transaction>,
        cursor_map: &mut HashMap<Felt, (Felt, u64)>,
    ) -> Result<()> {
        let mut unique_contracts = HashSet::new();
        // Contract -> Cursor
        for (event_idx, event) in events.iter().enumerate() {
            // NOTE: erc* processors expect the event_id to be in this format to get
            // transaction_hash:
            let event_id =
                format!("{:#064x}:{:#x}:{:#04x}", block_number, transaction_hash, event_idx);

            let event = Event {
                from_address: event.from_address,
                keys: event.keys.clone(),
                data: event.data.clone(),
            };

            let Some(&contract_type) = self.contracts.get(&event.from_address) else {
                continue;
            };

            unique_contracts.insert(event.from_address);

            self.process_event(
                block_number,
                block_timestamp,
                &event_id,
                &event,
                transaction_hash,
                contract_type,
            )
            .await?;
        }

        for contract in &unique_contracts {
            let entry = cursor_map.entry(*contract).or_insert((transaction_hash, 0));
            entry.0 = transaction_hash;
            entry.1 += 1;
        }

        if let Some(ref transaction) = transaction {
            Self::process_transaction(
                self,
                block_number,
                block_timestamp,
                transaction_hash,
                &unique_contracts,
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
        cursor_map: &mut HashMap<Felt, (Felt, u64)>,
    ) -> Result<()> {
        let transaction_hash = transaction_with_receipt.transaction.transaction_hash();
        let events = match &transaction_with_receipt.receipt {
            TransactionReceipt::Invoke(receipt) => Some(&receipt.events),
            TransactionReceipt::L1Handler(receipt) => Some(&receipt.events),
            _ => None,
        };

        let mut unique_contracts = HashSet::new();
        if let Some(events) = events {
            for (event_idx, event) in events.iter().enumerate() {
                let Some(&contract_type) = self.contracts.get(&event.from_address) else {
                    continue;
                };

                unique_contracts.insert(event.from_address);

                // NOTE: erc* processors expect the event_id to be in this format to get
                // transaction_hash:
                let event_id =
                    format!("{:#064x}:{:#x}:{:#04x}", block_number, *transaction_hash, event_idx);

                self.process_event(
                    block_number,
                    block_timestamp,
                    &event_id,
                    event,
                    *transaction_hash,
                    contract_type,
                )
                .await?;
            }

            if self.config.flags.contains(IndexingFlags::TRANSACTIONS) {
                self.process_transaction(
                    block_number,
                    block_timestamp,
                    *transaction_hash,
                    &unique_contracts,
                    &transaction_with_receipt.transaction,
                )
                .await?;
            }
        }

        for contract in unique_contracts {
            let entry = cursor_map.entry(contract).or_insert((*transaction_hash, 0));
            entry.0 = *transaction_hash;
            entry.1 += 1;
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
        contract_addresses: &HashSet<Felt>,
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
                    contract_addresses,
                    transaction,
                    self.contract_class_cache.as_ref(),
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
        contract_type: ContractType,
    ) -> Result<()> {
        if self.config.flags.contains(IndexingFlags::RAW_EVENTS) {
            self.db.store_event(event_id, event, transaction_hash, block_timestamp)?;
        }

        let event_key = event.keys[0];

        let processors = self.processors.get_event_processor(contract_type);
        let Some(processors) = processors.get(&event_key) else {
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
                        &self.config.event_processor_config,
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

        let processor = processors
            .iter()
            .find(|p| p.validate(event))
            .expect("Must find atleast one processor for the event");

        let (task_priority, task_identifier) =
            (processor.task_priority(), processor.task_identifier(event));

        // if our event can be parallelized, we add it to the task manager
        if task_identifier != task_manager::TASK_ID_SEQUENTIAL {
            self.task_manager.add_parallelized_event(
                task_priority,
                task_identifier,
                ParallelizedEvent {
                    contract_type,
                    event_id: event_id.to_string(),
                    event: event.clone(),
                    block_number,
                    block_timestamp,
                },
            );
        } else {
            // Process non-parallelized events immediately
            // if we dont have a task identifier, we process the event immediately
            if processor.validate(event) {
                if let Err(e) = processor
                    .process(
                        &self.world,
                        &mut self.db,
                        block_number,
                        block_timestamp,
                        event_id,
                        event,
                        &self.config.event_processor_config,
                    )
                    .await
                {
                    error!(target: LOG_TARGET, event_name = processor.event_key(), error = ?e, "Processing event.");
                }
            } else {
                warn!(target: LOG_TARGET, event_name = processor.event_key(), "Event not validated.");
            }
        }

        Ok(())
    }
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

// event_id format: block_number:transaction_hash:event_idx
pub fn get_transaction_hash_from_event_id(event_id: &str) -> String {
    event_id.split(':').nth(1).unwrap().to_string()
}
