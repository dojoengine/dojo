//! Server implementation for the Starknet JSON-RPC API.

mod read;
mod trace;
mod write;

use std::cmp::Ordering;
use std::ops::RangeInclusive;
use std::sync::Arc;

use anyhow::Context;
use katana_core::backend::Backend;
use katana_core::service::block_producer::{BlockProducer, BlockProducerMode, PendingExecutor};
use katana_executor::{ExecutionResult, ExecutorFactory};
use katana_pool::validation::stateful::TxValidator;
use katana_pool::TxPool;
use katana_primitives::block::{
    BlockHash, BlockHashOrNumber, BlockIdOrTag, BlockNumber, BlockTag, FinalityStatus,
};
use katana_primitives::class::{ClassHash, CompiledClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::conversion::rpc::legacy_inner_to_rpc_class;
use katana_primitives::env::BlockEnv;
use katana_primitives::event::ContinuationToken;
use katana_primitives::receipt::Event;
use katana_primitives::transaction::{ExecutableTxWithHash, TxHash, TxWithHash};
use katana_primitives::FieldElement;
use katana_provider::traits::block::{
    BlockHashProvider, BlockIdReader, BlockNumberProvider, BlockProvider,
};
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::FeeEstimate;
use katana_tasks::{BlockingTaskPool, TokioTaskSpawner};
use starknet::core::types::{
    ContractClass, EmittedEvent, EventsPage, TransactionExecutionStatus, TransactionStatus,
};

#[allow(missing_debug_implementations)]
pub struct StarknetApi<EF: ExecutorFactory> {
    inner: Arc<Inner<EF>>,
}

impl<EF: ExecutorFactory> Clone for StarknetApi<EF> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

struct Inner<EF: ExecutorFactory> {
    validator: TxValidator,
    pool: TxPool,
    backend: Arc<Backend<EF>>,
    block_producer: Arc<BlockProducer<EF>>,
    blocking_task_pool: BlockingTaskPool,
}

impl<EF: ExecutorFactory> StarknetApi<EF> {
    pub fn new(
        backend: Arc<Backend<EF>>,
        pool: TxPool,
        block_producer: Arc<BlockProducer<EF>>,
        validator: TxValidator,
    ) -> Self {
        let blocking_task_pool =
            BlockingTaskPool::new().expect("failed to create blocking task pool");

        let inner = Inner { pool, backend, block_producer, blocking_task_pool, validator };

        Self { inner: Arc::new(inner) }
    }

    async fn on_cpu_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        self.inner.blocking_task_pool.spawn(move || func(this)).await.unwrap()
    }

    async fn on_io_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        TokioTaskSpawner::new().unwrap().spawn_blocking(move || func(this)).await.unwrap()
    }

    fn estimate_fee_with(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        block_id: BlockIdOrTag,
        flags: katana_executor::SimulationFlag,
    ) -> Result<Vec<FeeEstimate>, StarknetApiError> {
        // get the state and block env at the specified block for execution
        let state = self.state(&block_id)?;
        let env = self.block_env_at(&block_id)?;

        // create the executor
        let executor = self.inner.backend.executor_factory.with_state_and_block_env(state, env);
        let results = executor.estimate_fee(transactions, flags);

        let mut estimates = Vec::with_capacity(results.len());
        for (i, res) in results.into_iter().enumerate() {
            match res {
                Ok(fee) => estimates.push(FeeEstimate {
                    gas_price: fee.gas_price.into(),
                    gas_consumed: fee.gas_consumed.into(),
                    overall_fee: fee.overall_fee.into(),
                    unit: fee.unit,
                    data_gas_price: Default::default(),
                    data_gas_consumed: Default::default(),
                }),

                Err(err) => {
                    return Err(StarknetApiError::TransactionExecutionError {
                        transaction_index: i,
                        execution_error: err.to_string(),
                    });
                }
            }
        }

        Ok(estimates)
    }

    /// Returns the pending state if the sequencer is running in _interval_ mode. Otherwise `None`.
    fn pending_executor(&self) -> Option<PendingExecutor> {
        match &*self.inner.block_producer.producer.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.executor()),
        }
    }

    fn state(&self, block_id: &BlockIdOrTag) -> Result<Box<dyn StateProvider>, StarknetApiError> {
        let provider = self.inner.backend.blockchain.provider();

        let state = match block_id {
            BlockIdOrTag::Tag(BlockTag::Latest) => Some(provider.latest()?),

            BlockIdOrTag::Tag(BlockTag::Pending) => {
                if let Some(exec) = self.pending_executor() {
                    Some(exec.read().state())
                } else {
                    Some(provider.latest()?)
                }
            }

            BlockIdOrTag::Hash(hash) => provider.historical((*hash).into())?,
            BlockIdOrTag::Number(num) => provider.historical((*num).into())?,
        };

        state.ok_or(StarknetApiError::BlockNotFound)
    }

    fn block_env_at(&self, block_id: &BlockIdOrTag) -> Result<BlockEnv, StarknetApiError> {
        let provider = self.inner.backend.blockchain.provider();

        let env = match block_id {
            BlockIdOrTag::Tag(BlockTag::Pending) => {
                if let Some(exec) = self.pending_executor() {
                    Some(exec.read().block_env())
                } else {
                    let num = provider.latest_number()?;
                    provider.block_env_at(num.into())?
                }
            }

            BlockIdOrTag::Tag(BlockTag::Latest) => {
                let num = provider.latest_number()?;
                provider.block_env_at(num.into())?
            }

            BlockIdOrTag::Hash(hash) => provider.block_env_at((*hash).into())?,
            BlockIdOrTag::Number(num) => provider.block_env_at((*num).into())?,
        };

        env.ok_or(StarknetApiError::BlockNotFound)
    }

    fn block_hash_and_number(&self) -> Result<(BlockHash, BlockNumber), StarknetApiError> {
        let provider = self.inner.backend.blockchain.provider();
        let hash = provider.latest_hash()?;
        let number = provider.latest_number()?;
        Ok((hash, number))
    }

    async fn class_at_hash(
        &self,
        block_id: BlockIdOrTag,
        class_hash: ClassHash,
    ) -> Result<ContractClass, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            let state = this.state(&block_id)?;

            let Some(class) = state.class(class_hash)? else {
                return Err(StarknetApiError::ClassHashNotFound);
            };

            match class {
                CompiledClass::Deprecated(class) => Ok(legacy_inner_to_rpc_class(class)?),
                CompiledClass::Class(_) => {
                    let Some(sierra) = state.sierra_class(class_hash)? else {
                        return Err(StarknetApiError::UnexpectedError {
                            reason: "Class hash exist, but its Sierra class is missing".to_string(),
                        });
                    };

                    Ok(ContractClass::Sierra(sierra))
                }
            }
        })
        .await
    }

    async fn class_hash_at_address(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            let state = this.state(&block_id)?;
            let class_hash = state.class_hash_of_contract(contract_address)?;
            class_hash.ok_or(StarknetApiError::ContractNotFound)
        })
        .await
    }

    async fn class_at_address(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> Result<ContractClass, StarknetApiError> {
        let hash = self.class_hash_at_address(block_id, contract_address).await?;
        let class = self.class_at_hash(block_id, hash).await?;
        Ok(class)
    }

    fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockIdOrTag,
    ) -> Result<StorageValue, StarknetApiError> {
        let state = self.state(&block_id)?;

        // check that contract exist by checking the class hash of the contract
        let Some(_) = state.class_hash_of_contract(contract_address)? else {
            return Err(StarknetApiError::ContractNotFound);
        };

        let value = state.storage(contract_address, storage_key)?;
        Ok(value.unwrap_or_default())
    }

    fn block_tx_count(&self, block_id: BlockIdOrTag) -> Result<u64, StarknetApiError> {
        let provider = self.inner.backend.blockchain.provider();

        let block_id: BlockHashOrNumber = match block_id {
            BlockIdOrTag::Tag(BlockTag::Pending) => match self.pending_executor() {
                Some(exec) => return Ok(exec.read().transactions().len() as u64),
                None => provider.latest_hash()?.into(),
            },
            BlockIdOrTag::Tag(BlockTag::Latest) => provider.latest_number()?.into(),
            BlockIdOrTag::Number(num) => num.into(),
            BlockIdOrTag::Hash(hash) => hash.into(),
        };

        let count = provider
            .transaction_count_by_block(block_id)?
            .ok_or(StarknetApiError::BlockNotFound)?;

        Ok(count)
    }

    async fn latest_block_number(&self) -> Result<BlockNumber, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            Ok(this.inner.backend.blockchain.provider().latest_number()?)
        })
        .await
    }

    async fn nonce_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> Result<Nonce, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            // read from the pool state if pending block
            //
            // TODO: this is a temporary solution, we should have a better way to handle this.
            // perhaps a pending/pool state provider that implements all the state provider traits.
            let result = if let BlockIdOrTag::Tag(BlockTag::Pending) = block_id {
                this.inner.validator.pool_nonce(contract_address)?
            } else {
                let state = this.state(&block_id)?;
                state.nonce(contract_address)?
            };

            let nonce = result.ok_or(StarknetApiError::ContractNotFound)?;
            Ok(nonce)
        })
        .await
    }

    async fn transaction(&self, hash: TxHash) -> Result<TxWithHash, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            let tx = this.inner.backend.blockchain.provider().transaction_by_hash(hash)?;

            let tx = match tx {
                tx @ Some(_) => tx,
                None => {
                    // check if the transaction is in the pending block
                    this.pending_executor().as_ref().and_then(|exec| {
                        exec.read()
                            .transactions()
                            .iter()
                            .find(|(tx, _)| tx.hash == hash)
                            .map(|(tx, _)| tx.clone())
                    })
                }
            };

            tx.ok_or(StarknetApiError::TxnHashNotFound)
        })
        .await
    }

    // TODO: should document more and possible find a simpler solution(?)
    fn events(
        &self,
        from_block: BlockIdOrTag,
        to_block: BlockIdOrTag,
        address: Option<ContractAddress>,
        keys: Option<Vec<Vec<FieldElement>>>,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> Result<EventsPage, StarknetApiError> {
        let provider = self.inner.backend.blockchain.provider();

        enum EventBlockId {
            Pending,
            Num(BlockNumber),
        }

        let from = if BlockIdOrTag::Tag(BlockTag::Pending) == from_block {
            EventBlockId::Pending
        } else {
            let num = provider.convert_block_id(from_block)?;
            EventBlockId::Num(num.ok_or(StarknetApiError::BlockNotFound)?)
        };

        let to = if BlockIdOrTag::Tag(BlockTag::Pending) == to_block {
            EventBlockId::Pending
        } else {
            let num = provider.convert_block_id(to_block)?;
            EventBlockId::Num(num.ok_or(StarknetApiError::BlockNotFound)?)
        };

        let mut cursor = match continuation_token {
            Some(token) => ContinuationToken::parse(&token)?,
            None => ContinuationToken::default(),
        };

        // reserved buffer to fill up with events to avoid reallocations
        let mut buffer = Vec::with_capacity(chunk_size as usize);
        let filter = EventFilter { address, keys };

        match (from, to) {
            (EventBlockId::Num(from), EventBlockId::Num(to)) => {
                let end = fill_events_at(
                    from..=to,
                    filter,
                    chunk_size,
                    provider,
                    &mut cursor,
                    &mut buffer,
                )?;

                // if we have exhausted all events in the requested range,
                // we don't need to return a continuation token anymore.
                if end {
                    return Ok(EventsPage { events: buffer, continuation_token: None });
                }
            }

            (EventBlockId::Num(from), EventBlockId::Pending) => {
                let latest = provider.latest_number()?;

                // if the cursor points to a block that is already past the latest block (ie
                // pending), we skip processing historical events.
                if cursor.block_n <= latest {
                    let end = fill_events_at(
                        from..=latest,
                        filter.clone(),
                        chunk_size,
                        provider,
                        &mut cursor,
                        &mut buffer,
                    )?;

                    if end && buffer.len() as u64 == chunk_size {
                        return Ok(EventsPage {
                            events: buffer,
                            continuation_token: Some(cursor.to_string()),
                        });
                    }
                }

                if let Some(executor) = self.pending_executor() {
                    fill_pending_events(&executor, filter, chunk_size, &mut cursor, &mut buffer)?;
                }
            }

            (EventBlockId::Pending, EventBlockId::Pending) => {
                if let Some(executor) = self.pending_executor() {
                    fill_pending_events(&executor, filter, chunk_size, &mut cursor, &mut buffer)?;
                }
            }

            (EventBlockId::Pending, EventBlockId::Num(_)) => {
                return Err(StarknetApiError::UnexpectedError {
                    reason: "Invalid block range".to_string(),
                });
            }
        }

        Ok(EventsPage { events: buffer, continuation_token: Some(cursor.to_string()) })
    }

    async fn transaction_status(
        &self,
        hash: TxHash,
    ) -> Result<TransactionStatus, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.backend.blockchain.provider();
            let status = provider.transaction_status(hash)?;

            if let Some(status) = status {
                // TODO: this might not work once we allow querying for 'failed' transactions from
                // the provider
                let Some(receipt) = provider.receipt_by_hash(hash)? else {
                    return Err(StarknetApiError::UnexpectedError {
                        reason: "Transaction hash exist, but the receipt is missing".to_string(),
                    });
                };

                let exec_status = if receipt.is_reverted() {
                    TransactionExecutionStatus::Reverted
                } else {
                    TransactionExecutionStatus::Succeeded
                };

                return Ok(match status {
                    FinalityStatus::AcceptedOnL1 => TransactionStatus::AcceptedOnL1(exec_status),
                    FinalityStatus::AcceptedOnL2 => TransactionStatus::AcceptedOnL2(exec_status),
                });
            }

            // seach in the pending block if the transaction is not found
            if let Some(pending_executor) = this.pending_executor() {
                let pending_executor = pending_executor.read();
                let pending_txs = pending_executor.transactions();
                let (_, res) = pending_txs
                    .iter()
                    .find(|(tx, _)| tx.hash == hash)
                    .ok_or(StarknetApiError::TxnHashNotFound)?;

                // TODO: should impl From<ExecutionResult> for TransactionStatus
                let status = match res {
                    ExecutionResult::Failed { .. } => TransactionStatus::Rejected,
                    ExecutionResult::Success { receipt, .. } => {
                        if receipt.is_reverted() {
                            TransactionStatus::AcceptedOnL2(TransactionExecutionStatus::Reverted)
                        } else {
                            TransactionStatus::AcceptedOnL2(TransactionExecutionStatus::Succeeded)
                        }
                    }
                };

                Ok(status)
            } else {
                Err(StarknetApiError::TxnHashNotFound)
            }
        })
        .await
    }
}

fn fill_pending_events(
    pending_executor: &PendingExecutor,
    filter: EventFilter,
    chunk_size: u64,
    cursor: &mut ContinuationToken,
    buffer: &mut Vec<EmittedEvent>,
) -> Result<(), StarknetApiError> {
    let block = pending_executor.read();
    let txs = block.transactions();

    // process indiviual transactions in the block
    for (i, (tx, res)) in txs.iter().enumerate().skip(cursor.txn_n as usize) {
        cursor.txn_n = i as u64;

        if let ExecutionResult::Success { receipt, .. } = res {
            let events = receipt.events();
            let tx_events_len = events.len();

            // check if cursor.event_n is correct
            match (tx_events_len as u64).cmp(&cursor.event_n) {
                Ordering::Less => {
                    return Err(StarknetApiError::InvalidContinuationToken);
                }
                Ordering::Equal => {
                    cursor.txn_n += 1;
                    cursor.event_n = 0;
                    continue;
                }
                Ordering::Greater => (),
            }

            // calculate the remaining capacity based on the chunk size and the current
            // number of events we have taken.
            let total_can_take = (chunk_size as usize).saturating_sub(tx_events_len);

            // skip events according to the continuation token.
            let filtered = filter_events(events.iter(), filter.clone())
                .enumerate()
                .skip(cursor.event_n as usize)
                .take(total_can_take)
                .map(|(i, e)| {
                    (
                        i,
                        EmittedEvent {
                            block_hash: None,
                            block_number: None,
                            keys: e.keys.clone(),
                            data: e.data.clone(),
                            transaction_hash: tx.hash,
                            from_address: e.from_address.into(),
                        },
                    )
                })
                .collect::<Vec<_>>();

            // remaining possible events that we haven't seen due to the chunk size limit.
            let chunk_seen_end = cursor.event_n as usize + total_can_take;
            // get the index of the last matching event that we have reached. if there is not
            // matching events (ie `filtered` is empty) we point the end of the chunk
            // we've covered thus far..
            let last_event_idx = filtered.last().map(|(i, _)| *i).unwrap_or(chunk_seen_end);
            // the next time we have to fetch the events, we will start from this index.
            let new_event_n = if total_can_take == 0 {
                // if we haven't taken any events, due to the chunk size limit, we need to start
                // from the the same event pointed by the current cursor..
                cursor.event_n as usize + last_event_idx
            } else {
                // start at the next event of the last event we've filtered out.
                cursor.event_n as usize + last_event_idx + 1
            };

            buffer.extend(filtered.into_iter().map(|(_, event)| event));

            // if there are still more events that we haven't fetched yet for this tx.
            if new_event_n < tx_events_len {
                cursor.event_n = new_event_n as u64;
            }
            // reset the event index
            else {
                cursor.txn_n += 1;
                cursor.event_n = 0;
            }

            if buffer.len() >= chunk_size as usize {
                break;
            }
        }
    }

    Ok(())
}

/// Returns `true` if reach the end of the block range.
fn fill_events_at<P>(
    block_range: RangeInclusive<BlockNumber>,
    filter: EventFilter,
    chunk_size: u64,
    provider: P,
    cursor: &mut ContinuationToken,
    buffer: &mut Vec<EmittedEvent>,
) -> Result<bool, StarknetApiError>
where
    P: BlockProvider + ReceiptProvider,
{
    // update the block range to start from the block pointed by the cursor.
    let range = (block_range.start() + cursor.block_n)..=*block_range.end();

    for block_num in range {
        cursor.block_n = block_num;

        let block_hash = provider.block_hash_by_num(block_num)?.context("Missing block hash")?;
        let receipts = provider.receipts_by_block(block_num.into())?.context("Missing receipts")?;
        let tx_range =
            provider.block_body_indices(block_num.into())?.context("Missing block body index")?;

        let tx_hashes = provider.transaction_hashes_in_range(tx_range.into())?;
        let total_txn = receipts.len();

        // the cursor.txn_n indicates from which transaction to start, so
        // if the value is larger than the total number of transactions available in the
        // block.
        if (total_txn as u64) < cursor.txn_n {
            return Err(StarknetApiError::InvalidContinuationToken);
        }

        // skip number of transactions as specified in the continuation token
        for (i, (tx_hash, events)) in tx_hashes
            .into_iter()
            .zip(receipts.iter().map(|r| r.events()))
            .enumerate()
            .skip(cursor.txn_n as usize)
        {
            cursor.txn_n = i as u64;

            // the total number of events in the transaction
            let tx_events_len: usize = events.len();

            // check if cursor.event_n is correct
            match tx_events_len.cmp(&(cursor.event_n as usize)) {
                Ordering::Less => {
                    return Err(StarknetApiError::InvalidContinuationToken);
                }
                Ordering::Equal => {
                    cursor.txn_n += 1;
                    cursor.event_n = 0;
                    continue;
                }
                Ordering::Greater => (),
            }

            // calculate the remaining capacity based on the chunk size and the current
            // number of events we have taken.
            let total_can_take = (chunk_size as usize).saturating_sub(tx_events_len);

            // skip events according to the continuation token.
            let filtered = filter_events(events.iter(), filter.clone())
                .enumerate()
                .skip(cursor.event_n as usize)
                .take(total_can_take)
                .map(|(i, e)| {
                    (
                        i,
                        EmittedEvent {
                            keys: e.keys.clone(),
                            data: e.data.clone(),
                            transaction_hash: tx_hash,
                            block_number: Some(block_num),
                            block_hash: Some(block_hash),
                            from_address: e.from_address.into(),
                        },
                    )
                })
                .collect::<Vec<_>>();

            // remaining possible events that we haven't seen due to the chunk size limit.
            let chunk_seen_end = cursor.event_n as usize + total_can_take;
            // get the index of the last matching event that we have reached. if there is not
            // matching events (ie `filtered` is empty) we point the end of the chunk
            // we've covered thus far..
            let last_event_idx = filtered.last().map(|(i, _)| *i).unwrap_or(chunk_seen_end);
            // the next time we have to fetch the events, we will start from this index.
            let new_event_n = if total_can_take == 0 {
                // if we haven't taken any events, due to the chunk size limit, we need to start
                // from the the same event pointed by the current cursor..
                cursor.event_n as usize + last_event_idx
            } else {
                // start at the next event of the last event we've filtered out.
                cursor.event_n as usize + last_event_idx + 1
            };

            buffer.extend(filtered.into_iter().map(|(_, event)| event));

            if buffer.len() >= chunk_size as usize {
                // check if there are still more events that we haven't fetched yet. if yes,
                // we need to update the cursor to point to the next event to start fetching from
                // in the same transaction..
                if new_event_n < tx_events_len {
                    cursor.event_n = new_event_n as u64;
                }
                // when there are no more events to fetch in this transaction
                else {
                    cursor.event_n = 0;

                    // if there are still more transactions to fetch, we need to increment the
                    // transaction index.
                    if i + 1 < total_txn {
                        cursor.txn_n += 1;
                    }
                    // if we have reached the end of the block, point to the next block.
                    else if block_num == *block_range.end() {
                        cursor.block_n += 1;
                        cursor.txn_n = 0;
                        return Ok(true);
                    }
                }

                return Ok(false);
            }
        }
    }

    // reset the txn-scoped index
    cursor.txn_n = 0;
    cursor.event_n = 0;

    Ok(true)
}

/// An object to specify how events should be filtered.
#[derive(Debug, Default, Clone)]
struct EventFilter {
    /// The contract address to filter by.
    ///
    /// If `None`, all events are considered. If `Some`, only events emitted by the specified
    /// contract are considered.
    address: Option<ContractAddress>,
    /// The keys to filter by.
    keys: Option<Vec<Vec<FieldElement>>>,
}

/// An iterator that yields events that match the given filters.
#[derive(Debug)]
struct FilteredEvents<'a, I: Iterator<Item = &'a Event>> {
    iter: I,
    filter: EventFilter,
}

impl<'a, I: Iterator<Item = &'a Event>> FilteredEvents<'a, I> {
    fn new(iter: I, filter: EventFilter) -> Self {
        Self { iter, filter }
    }
}

impl<'a, I: Iterator<Item = &'a Event>> Iterator for FilteredEvents<'a, I> {
    type Item = &'a Event;

    fn next(&mut self) -> Option<Self::Item> {
        for event in self.iter.by_ref() {
            // Check if the event matches the address filter
            if !self.filter.address.map_or(true, |addr| addr == event.from_address) {
                continue;
            }

            // Check if the event matches the keys filter
            let is_matched = match &self.filter.keys {
                None => true,
                // From starknet-api spec:
                // Per key (by position), designate the possible values to be matched for events to
                // be returned. Empty array designates 'any' value"
                Some(filters) => filters.iter().enumerate().all(|(i, keys)| {
                    // Lets say we want to filter events which are either named `Event1` or `Event2`
                    // and custom key `0x1` or `0x2` Filter:
                    // [[sn_keccak("Event1"), sn_keccak("Event2")], ["0x1", "0x2"]]

                    // This checks: number of keys in event >= number of keys in filter (we check >
                    // i and not >= i because i is zero indexed) because
                    // otherwise this event doesn't contain all the keys we
                    // requested
                    event.keys.len() > i &&
                         // This checks: Empty array desginates 'any' value
                         (keys.is_empty()
                         ||
                         // This checks: If this events i'th value is one of the requested value in filter_keys[i]
                         keys.contains(&event.keys[i]))
                }),
            };

            if is_matched {
                return Some(event);
            }
        }

        None
    }
}

fn filter_events<'a, I: Iterator<Item = &'a Event>>(
    events: I,
    filter: EventFilter,
) -> FilteredEvents<'a, I> {
    FilteredEvents::new(events, filter)
}
