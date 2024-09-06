use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::time::Duration;

use anyhow::Result;
use dojo_world::contracts::world::WorldContractReader;
use hashlink::LinkedHashMap;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, Event, EventFilter, Felt, MaybePendingBlockWithReceipts,
    MaybePendingBlockWithTxHashes, PendingBlockWithReceipts, ReceiptBlock, TransactionReceipt,
    TransactionReceiptWithBlockInfo, TransactionWithReceipt,
};
use starknet::providers::Provider;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Sender as BoundedSender;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

use crate::processors::event_message::EventMessageProcessor;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::sql::Sql;

#[allow(missing_debug_implementations)]
pub struct Processors<P: Provider + Send + Sync + std::fmt::Debug> {
    pub block: Vec<Box<dyn BlockProcessor<P>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<P>>>,
    pub event: HashMap<Felt, Box<dyn EventProcessor<P>>>,
    pub catch_all_event: Box<dyn EventProcessor<P>>,
}

impl<P: Provider + Send + Sync + std::fmt::Debug> Default for Processors<P> {
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

#[derive(Debug)]
pub struct EngineConfig {
    pub polling_interval: Duration,
    pub start_block: u64,
    pub events_chunk_size: u64,
    pub index_pending: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_millis(500),
            start_block: 0,
            events_chunk_size: 1024,
            index_pending: true,
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

#[allow(missing_debug_implementations)]
pub struct Engine<P: Provider + Send + Sync + std::fmt::Debug> {
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

impl<P: Provider + Send + Sync + std::fmt::Debug> Engine<P> {
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
        // use the start block provided by user if head is 0
        let (head, last_pending_block_world_tx, last_pending_block_tx) = self.db.head().await?;
        if head == 0 {
            self.db.set_head(
                self.config.start_block,
                last_pending_block_world_tx,
                last_pending_block_tx,
            );
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

    pub async fn fetch_data(
        &mut self,
        from: u64,
        last_pending_block_world_tx: Option<Felt>,
        last_pending_block_tx: Option<Felt>,
    ) -> Result<FetchDataResult> {
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        let result = if from < latest_block_number {
            let from = if from == 0 { from } else { from + 1 };
            debug!(target: LOG_TARGET, from = %from, to = %latest_block_number, "Fetching data for range.");
            let data =
                self.fetch_range(from, latest_block_number, last_pending_block_world_tx).await?;
            FetchDataResult::Range(data)
        } else if self.config.index_pending {
            let data = self.fetch_pending(latest_block_number + 1, last_pending_block_tx).await?;
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
                            self.db.set_head(
                                data.block_number - 1,
                                last_pending_block_world_tx,
                                last_pending_block_tx,
                            );
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
                    info!(target: LOG_TARGET, transaction_hash = %format!("{:#x}", transaction_hash), "Processed pending transaction.")
                }
            }
        }

        // Set the head to the last processed pending transaction
        // Head block number should still be latest block number
        self.db.set_head(data.block_number - 1, last_pending_block_world_tx, last_pending_block_tx);

        self.db.execute().await?;
        Ok(())
    }

    pub async fn process_range(&mut self, data: FetchRangeResult) -> Result<()> {
        // Process all transactions
        let mut last_block = 0;
        for ((block_number, transaction_hash), events) in data.transactions {
            debug!("Processing transaction hash: {:#x}", transaction_hash);
            // Process transaction
            // let transaction = self.provider.get_transaction_by_hash(transaction_hash).await?;

            self.process_transaction_with_events(
                transaction_hash,
                events.as_slice(),
                block_number,
                data.blocks[&block_number],
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

            if self.db.query_queue.queue.len() >= QUERY_QUEUE_BATCH_SIZE {
                self.db.execute().await?;
            }
        }

        // We return None for the pending_block_tx because our process_range
        // gets only specific events from the world. so some transactions
        // might get ignored and wont update the cursor.
        // so once the sync range is done, we assume all of the tx of the block
        // have been processed.

        self.db.set_head(data.latest_block_number, None, None);
        self.db.execute().await?;

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

        // Commented out this transaction processor because it requires an RPC call for each
        // transaction which is slowing down the sync process by alot.
        // Self::process_transaction(
        //     self,
        //     block_number,
        //     block_timestamp,
        //     transaction_hash,
        //     transaction,
        // )
        // .await?;

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

            // if world_event {
            //     Self::process_transaction(
            //         self,
            //         block_number,
            //         block_timestamp,
            //         transaction_hash,
            //         transaction,
            //     )
            //     .await?;
            // }
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

    // async fn process_transaction(
    //     &mut self,
    //     block_number: u64,
    //     block_timestamp: u64,
    //     transaction_hash: Felt,
    //     transaction: &Transaction,
    // ) -> Result<()> {
    //     for processor in &self.processors.transaction {
    //         processor
    //             .process(
    //                 &mut self.db,
    //                 self.provider.as_ref(),
    //                 block_number,
    //                 block_timestamp,
    //                 transaction_hash,
    //                 transaction,
    //             )
    //             .await?
    //     }

    //     Ok(())
    // }

    async fn process_event(
        &mut self,
        block_number: u64,
        block_timestamp: u64,
        event_id: &str,
        event: &Event,
        transaction_hash: Felt,
    ) -> Result<()> {
        self.db.store_event(event_id, event, transaction_hash, block_timestamp);
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

        // if processor.validate(event) {
        if let Err(e) = processor
            .process(&self.world, &mut self.db, block_number, block_timestamp, event_id, event)
            .await
        {
            error!(target: LOG_TARGET, event_name = processor.event_key(), error = %e, "Processing event.");
        }
        // }

        Ok(())
    }
}
