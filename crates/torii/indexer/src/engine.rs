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
use starknet::core::types::requests::{
    GetBlockWithTxHashesRequest, GetEventsRequest, GetTransactionByHashRequest,
};
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BlockTag, DeclareTransaction, DeclareTransactionContent,
    DeclareTransactionV0, DeclareTransactionV1, DeclareTransactionV2, DeclareTransactionV3,
    DeployAccountTransaction, DeployAccountTransactionContent, DeployAccountTransactionV1,
    DeployAccountTransactionV3, DeployTransaction, EmittedEvent, Event, EventFilter,
    EventFilterWithPage, InvokeTransaction, InvokeTransactionContent, InvokeTransactionV0,
    InvokeTransactionV1, InvokeTransactionV3, L1HandlerTransaction, MaybePendingBlockWithReceipts,
    MaybePendingBlockWithTxHashes, PendingBlockWithReceipts, ResultPageRequest, Transaction,
    TransactionContent, TransactionReceipt, TransactionWithReceipt,
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
            FetchDataResult::Range(range) => {
                Some(BlockId::Number(*range.blocks.keys().last().unwrap()))
            }
            FetchDataResult::Pending(_pending) => Some(BlockId::Tag(BlockTag::Pending)),
            FetchDataResult::None => None,
        }
    }
}

#[derive(Debug)]
pub struct FetchRangeTransaction {
    // this is Some if the transactions indexing flag
    // is enabled
    pub transaction: Option<Transaction>,
    pub events: Vec<EmittedEvent>,
}

#[derive(Debug)]
pub struct FetchRangeResult {
    // block_number -> (transaction_hash -> events)
    pub transactions: BTreeMap<u64, LinkedHashMap<Felt, FetchRangeTransaction>>,
    // block_number -> block_timestamp
    pub blocks: BTreeMap<u64, u64>,
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

    pub async fn fetch_data(&mut self, cursors: &Cursors) -> Result<FetchDataResult> {
        let latest_block = self.provider.block_hash_and_number().await?;
        let from = cursors.head.unwrap_or(self.config.world_block);
        // this is non-inclusive. this just means that we stop doing events pages fetches once we
        // reach a page with an event that is after the latest block. so in our final
        // commit; we could end up with a higher head than this one.
        let to = latest_block.block_number.min(from + self.config.blocks_chunk_size);

        let instant = Instant::now();
        let result = if from < latest_block.block_number {
            let from = if from == 0 { from } else { from + 1 };

            // Fetch all events from 'from' to our blocks chunk size
            let range =
                self.fetch_range(from, to, &cursors.cursor_map, latest_block.block_number).await?;

            debug!(target: LOG_TARGET, duration = ?instant.elapsed(), from = %from, to = %range.blocks.keys().last().unwrap(), "Fetched data for range.");
            FetchDataResult::Range(range)
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
        &self,
        from: u64,
        to: u64,
        cursor_map: &HashMap<Felt, Felt>,
        latest_block_number: u64,
    ) -> Result<FetchRangeResult> {
        let mut events = vec![];

        // Create initial batch requests for all contracts
        let mut event_requests = Vec::new();
        for (contract_address, _) in self.contracts.iter() {
            let events_filter = EventFilter {
                from_block: Some(BlockId::Number(from)),
                to_block: None,
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
        events.extend(self.fetch_events_recursive(event_requests, cursor_map, to).await?);

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
                .or_insert(FetchRangeTransaction { transaction: None, events: vec![] })
                .events
                .push(event);
        }

        // If transactions indexing flag is enabled, we should batch request all
        // of our recolted transactions
        if self.config.flags.contains(IndexingFlags::TRANSACTIONS) && !transactions.is_empty() {
            let mut transaction_requests = Vec::with_capacity(transactions.len());
            let mut block_numbers = Vec::with_capacity(transactions.len());
            for (block_number, transactions) in &transactions {
                for (transaction_hash, _) in transactions {
                    transaction_requests.push(ProviderRequestData::GetTransactionByHash(
                        GetTransactionByHashRequest { transaction_hash: *transaction_hash },
                    ));
                    block_numbers.push(*block_number);
                }
            }

            let transaction_results = self.provider.batch_requests(transaction_requests).await?;
            for (block_number, result) in block_numbers.into_iter().zip(transaction_results) {
                match result {
                    ProviderResponseData::GetTransactionByHash(transaction) => {
                        transactions.entry(block_number).and_modify(|txns| {
                            txns.entry(*transaction.transaction_hash())
                                .and_modify(|tx| tx.transaction = Some(transaction));
                        });
                    }
                    _ => unreachable!(),
                }
            }
        }

        // Always ensure the latest block number is included
        block_numbers.insert(to);

        // Batch request block timestamps
        let mut timestamp_requests = Vec::new();
        for block_number in &block_numbers {
            timestamp_requests.push(ProviderRequestData::GetBlockWithTxHashes(
                GetBlockWithTxHashesRequest {
                    block_id: if *block_number == latest_block_number {
                        BlockId::Tag(BlockTag::Latest)
                    } else {
                        BlockId::Number(*block_number)
                    },
                },
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
                    _ => unreachable!(),
                }
            }
        }

        trace!(target: LOG_TARGET, "Transactions: {}", &transactions.len());
        trace!(target: LOG_TARGET, "Blocks: {}", &blocks.len());

        Ok(FetchRangeResult { transactions, blocks })
    }

    #[async_recursion]
    async fn fetch_events_recursive(
        &self,
        requests: Vec<(Felt, ProviderRequestData)>,
        cursor_map: &HashMap<Felt, Felt>,
        to: u64,
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
            let last_contract_tx = cursor_map.get(&contract_address).cloned();
            let mut last_contract_tx_tmp = last_contract_tx;

            match result {
                ProviderResponseData::GetEvents(events_page) => {
                    let last_block_number =
                        events_page.events.last().map_or(0, |e| e.block_number.unwrap());

                    // Process events for this page, only including events up to our target block
                    for event in events_page.events {
                        let block_number = event.block_number.unwrap();
                        if block_number > to {
                            continue;
                        }

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

                    // Continue fetching pages if there are more events and we haven't seen all
                    // events up to our target block
                    if let Some(continuation_token) = events_page.continuation_token {
                        // Only continue if we haven't seen all events up to our target block
                        if last_block_number < to {
                            if let ProviderRequestData::GetEvents(mut next_request) =
                                original_request
                            {
                                next_request.filter.result_page_request.continuation_token =
                                    Some(continuation_token);
                                next_requests.push((
                                    contract_address,
                                    ProviderRequestData::GetEvents(next_request),
                                ));
                            }
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
            events.extend(self.fetch_events_recursive(next_requests, cursor_map, to).await?);
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
            FetchDataResult::Range(range) => self.process_range(range).await?,
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
            let transaction_hash = t.receipt.transaction_hash();
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

    pub async fn process_range(&mut self, range: FetchRangeResult) -> Result<()> {
        let mut processed_blocks = HashSet::new();
        let mut cursor_map = HashMap::new();

        // Process all transactions in the chunk
        for (block_number, transactions) in range.transactions {
            for (transaction_hash, tx) in transactions {
                trace!(target: LOG_TARGET, "Processing transaction hash: {:#x}", transaction_hash);

                self.process_transaction_with_events(
                    transaction_hash,
                    tx.events.as_slice(),
                    block_number,
                    range.blocks[&block_number],
                    tx.transaction,
                    &mut cursor_map,
                )
                .await?;
            }

            // Process block
            if !processed_blocks.contains(&block_number) {
                if let Some(ref block_tx) = self.block_tx {
                    block_tx.send(block_number).await?;
                }

                self.process_block(block_number, range.blocks[&block_number]).await?;
                processed_blocks.insert(block_number);
            }
        }

        // Process parallelized events
        self.task_manager.process_tasks().await?;

        let (last_block_number, last_block_timestamp) = range.blocks.iter().last().unwrap();
        self.db.update_cursors(*last_block_number, *last_block_timestamp, None, cursor_map)?;

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
        let transaction_hash = transaction_with_receipt.receipt.transaction_hash();
        let events = match &transaction_with_receipt.receipt {
            TransactionReceipt::Invoke(receipt) => Some(&receipt.events),
            TransactionReceipt::L1Handler(receipt) => Some(&receipt.events),
            _ => None,
        };

        let mut unique_contracts = HashSet::new();
        if let Some(events) = events {
            for (event_idx, event) in events.iter().enumerate() {
                // Skip events that are not from a contract we are indexing
                let Some(&contract_type) = self.contracts.get(&event.from_address) else {
                    continue;
                };

                unique_contracts.insert(event.from_address);

                // NOTE: erc* processors expect the event_id to be in this format to get
                // transaction_hash:
                let event_id: String =
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

            // Process transaction if we have at least one an event from a contract we are indexing
            if self.config.flags.contains(IndexingFlags::TRANSACTIONS)
                && !unique_contracts.is_empty()
            {
                self.process_transaction(
                    block_number,
                    block_timestamp,
                    *transaction_hash,
                    &unique_contracts,
                    &transaction_with_receipt_to_transaction(transaction_with_receipt),
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

        trace!(target: LOG_TARGET, block_number = %block_number, "Processed block.");
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

// event_id format: block_number:transaction_hash:event_idx
pub fn get_transaction_hash_from_event_id(event_id: &str) -> String {
    event_id.split(':').nth(1).unwrap().to_string()
}

// Conversion function due type changes when bumping `starknet` to 0.14.0. Prior to 0.14.0, the
// `transaction` field in `TransactionWithReceipt` was of type `Transaction`.
fn transaction_with_receipt_to_transaction(
    tx_with_receipt: &TransactionWithReceipt,
) -> Transaction {
    match &tx_with_receipt.transaction {
        TransactionContent::Invoke(invoke) => match invoke {
            InvokeTransactionContent::V0(content) => {
                Transaction::Invoke(InvokeTransaction::V0(InvokeTransactionV0 {
                    max_fee: content.max_fee,
                    calldata: content.calldata.clone(),
                    signature: content.signature.clone(),
                    contract_address: content.contract_address,
                    entry_point_selector: content.entry_point_selector,
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                }))
            }
            InvokeTransactionContent::V1(content) => {
                Transaction::Invoke(InvokeTransaction::V1(InvokeTransactionV1 {
                    nonce: content.nonce,
                    max_fee: content.max_fee,
                    calldata: content.calldata.clone(),
                    signature: content.signature.clone(),
                    sender_address: content.sender_address,
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                }))
            }
            InvokeTransactionContent::V3(content) => {
                Transaction::Invoke(InvokeTransaction::V3(InvokeTransactionV3 {
                    tip: content.tip,
                    nonce: content.nonce,
                    calldata: content.calldata.clone(),
                    signature: content.signature.clone(),
                    sender_address: content.sender_address,
                    paymaster_data: content.paymaster_data.clone(),
                    resource_bounds: content.resource_bounds.clone(),
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                    fee_data_availability_mode: content.fee_data_availability_mode,
                    account_deployment_data: content.account_deployment_data.clone(),
                    nonce_data_availability_mode: content.nonce_data_availability_mode,
                }))
            }
        },
        TransactionContent::Declare(declare) => match declare {
            DeclareTransactionContent::V0(content) => {
                Transaction::Declare(DeclareTransaction::V0(DeclareTransactionV0 {
                    max_fee: content.max_fee,
                    class_hash: content.class_hash,
                    signature: content.signature.clone(),
                    sender_address: content.sender_address,
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                }))
            }
            DeclareTransactionContent::V1(content) => {
                Transaction::Declare(DeclareTransaction::V1(DeclareTransactionV1 {
                    nonce: content.nonce,
                    max_fee: content.max_fee,
                    class_hash: content.class_hash,
                    signature: content.signature.clone(),
                    sender_address: content.sender_address,
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                }))
            }
            DeclareTransactionContent::V2(content) => {
                Transaction::Declare(DeclareTransaction::V2(DeclareTransactionV2 {
                    nonce: content.nonce,
                    max_fee: content.max_fee,
                    class_hash: content.class_hash,
                    signature: content.signature.clone(),
                    sender_address: content.sender_address,
                    compiled_class_hash: content.compiled_class_hash,
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                }))
            }
            DeclareTransactionContent::V3(content) => {
                Transaction::Declare(DeclareTransaction::V3(DeclareTransactionV3 {
                    tip: content.tip,
                    nonce: content.nonce,
                    class_hash: content.class_hash,
                    signature: content.signature.clone(),
                    sender_address: content.sender_address,
                    paymaster_data: content.paymaster_data.clone(),
                    compiled_class_hash: content.compiled_class_hash,
                    resource_bounds: content.resource_bounds.clone(),
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                    fee_data_availability_mode: content.fee_data_availability_mode,
                    account_deployment_data: content.account_deployment_data.clone(),
                    nonce_data_availability_mode: content.nonce_data_availability_mode,
                }))
            }
        },
        TransactionContent::DeployAccount(deploy_account) => match deploy_account {
            DeployAccountTransactionContent::V1(content) => Transaction::DeployAccount(
                DeployAccountTransaction::V1(DeployAccountTransactionV1 {
                    nonce: content.nonce,
                    max_fee: content.max_fee,
                    class_hash: content.class_hash,
                    signature: content.signature.clone(),
                    contract_address_salt: content.contract_address_salt,
                    constructor_calldata: content.constructor_calldata.clone(),
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                }),
            ),
            DeployAccountTransactionContent::V3(content) => Transaction::DeployAccount(
                DeployAccountTransaction::V3(DeployAccountTransactionV3 {
                    tip: content.tip,
                    nonce: content.nonce,
                    class_hash: content.class_hash,
                    signature: content.signature.clone(),
                    paymaster_data: content.paymaster_data.clone(),
                    resource_bounds: content.resource_bounds.clone(),
                    contract_address_salt: content.contract_address_salt,
                    constructor_calldata: content.constructor_calldata.clone(),
                    transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
                    fee_data_availability_mode: content.fee_data_availability_mode,
                    nonce_data_availability_mode: content.nonce_data_availability_mode,
                }),
            ),
        },
        TransactionContent::Deploy(content) => Transaction::Deploy(DeployTransaction {
            version: content.version,
            class_hash: content.class_hash,
            contract_address_salt: content.contract_address_salt,
            constructor_calldata: content.constructor_calldata.clone(),
            transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
        }),
        TransactionContent::L1Handler(content) => Transaction::L1Handler(L1HandlerTransaction {
            nonce: content.nonce,
            version: content.version,
            calldata: content.calldata.clone(),
            contract_address: content.contract_address,
            entry_point_selector: content.entry_point_selector,
            transaction_hash: *tx_with_receipt.receipt.transaction_hash(),
        }),
    }
}
