//! Server implementation for the Starknet JSON-RPC API.

mod read;
mod trace;
mod write;

use std::cmp::Ordering;
use std::sync::Arc;

use anyhow::Result;
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
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
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
        let mut current_block = 0;

        // convert block id to block number
        let from = provider.convert_block_id(from_block)?;
        let to = provider.convert_block_id(to_block)?;

        let is_from_pending = matches!(from_block, BlockIdOrTag::Tag(BlockTag::Pending));
        let is_to_pending = matches!(to_block, BlockIdOrTag::Tag(BlockTag::Pending));

        // If either of the requested blocks is None, and neither of them is a pending block
        // return an error.
        if (from.is_none() || to.is_none()) && !(is_from_pending || is_to_pending) {
            return Err(StarknetApiError::BlockNotFound);
        }

        let mut continuation_token = match continuation_token {
            Some(token) => ContinuationToken::parse(&token)?,
            None => ContinuationToken::default(),
        };

        let mut all_events = Vec::with_capacity(chunk_size as usize);

        if let (Some(from), Some(to)) = (from, to) {
            // skip blocks as specified in the continuation token
            let block_range = (from + continuation_token.block_n)..=to;

            for current in block_range {
                let block_hash = provider
                    .block_hash_by_num(current)?
                    .ok_or(StarknetApiError::UnexpectedError { reason: "Missing".into() })?;

                let receipts = provider
                    .receipts_by_block(current.into())?
                    .ok_or(StarknetApiError::UnexpectedError { reason: "Missing".into() })?;

                let tx_range = provider
                    .block_body_indices(current.into())?
                    .ok_or(StarknetApiError::UnexpectedError { reason: "Missing".into() })?;

                let tx_hashes = provider.transaction_hashes_in_range(tx_range.into())?;

                let txn_n = receipts.len();

                // the continuation_token.txn_n indicates from which transaction to start, so
                // if the value is larger than the total number of transactions available in the
                // block.
                if (txn_n as u64) < continuation_token.txn_n {
                    return Err(StarknetApiError::InvalidContinuationToken);
                }

                // skip number of transactions as specified in the continuation token
                for (tx_hash, events) in tx_hashes
                    .into_iter()
                    .zip(receipts.iter().map(|r| r.events()))
                    .skip(continuation_token.txn_n as usize)
                {
                    let txn_events_len: usize = events.len();

                    // check if continuation_token.event_n is correct
                    match (txn_events_len as u64).cmp(&continuation_token.event_n) {
                        Ordering::Less => {
                            return Err(StarknetApiError::InvalidContinuationToken);
                        }
                        Ordering::Equal => {
                            continuation_token.txn_n += 1;
                            continuation_token.event_n = 0;
                            continue;
                        }
                        Ordering::Greater => (),
                    }

                    // calculate the remaining capacity based on the chunk size and the current //
                    // number of events we have taken.
                    let remaining_capacity = chunk_size as usize - all_events.len();

                    // skip events according to the continuation token.
                    let filtered_events = filter_events(events.iter(), address, keys.clone())
                        .skip(continuation_token.event_n as usize)
                        .take(remaining_capacity)
                        .map(|e| EmittedEvent {
                            keys: e.keys.clone(),
                            data: e.data.clone(),
                            block_number: Some(current),
                            transaction_hash: tx_hash,
                            block_hash: Some(block_hash),
                            from_address: e.from_address.into(),
                        })
                        .collect::<Vec<_>>();

                    // the next time we have to fetch the events, we will start from this index.
                    let new_event_n = continuation_token.event_n as usize + filtered_events.len();

                    all_events.extend(filtered_events);

                    if all_events.len() >= chunk_size as usize {
                        // check if there are still more events that we haven't fetched yet. if yes,
                        // we need to return a continuation token that points to the next event to
                        // start fetching from.
                        let token = if current_block < to
                            || continuation_token.txn_n < txn_n as u64 - 1
                            || new_event_n < txn_events_len
                        {
                            continuation_token.event_n = new_event_n as u64;
                            Some(continuation_token.to_string())
                        } else {
                            None
                        };

                        return Ok(EventsPage { events: all_events, continuation_token: token });
                    }

                    // reset the event index and increment the transaction index.
                    continuation_token.txn_n += 1;
                    continuation_token.event_n = 0;
                }

                current_block += 1;
                continuation_token.txn_n = 0;
                continuation_token.block_n += 1;
            }
        }

        // TODO: refactor this to avoid code duplication
        // process the pending block
        if is_from_pending || is_to_pending {
            if let Some(pending_executor) = self.pending_executor() {
                let pending_block = pending_executor.read();
                let txs = pending_block.transactions();

                // process indiviual transactions in the block
                for (tx, res) in txs.iter().skip(continuation_token.txn_n as usize) {
                    if let ExecutionResult::Success { receipt, .. } = res {
                        let events = receipt.events();

                        let txn_events_len = events.len();

                        // check if continuation_token.event_n is correct
                        match (txn_events_len as u64).cmp(&continuation_token.event_n) {
                            Ordering::Less => {
                                return Err(StarknetApiError::InvalidContinuationToken);
                            }
                            Ordering::Equal => {
                                continuation_token.txn_n += 1;
                                continuation_token.event_n = 0;
                                continue;
                            }
                            Ordering::Greater => (),
                        }

                        // calculate the remaining capacity based on the chunk size and the current
                        // // number of events we have taken.
                        let remaining_capacity = chunk_size as usize - all_events.len();

                        // skip events according to the continuation token.
                        let filtered_events = filter_events(events.iter(), address, keys.clone())
                            .skip(continuation_token.event_n as usize)
                            .take(remaining_capacity)
                            .map(|e| EmittedEvent {
                                block_hash: None,
                                block_number: None,
                                keys: e.keys.clone(),
                                data: e.data.clone(),
                                transaction_hash: tx.hash,
                                from_address: e.from_address.into(),
                            })
                            .collect::<Vec<_>>();

                        // the next time we have to fetch the events, we will start from this index.
                        let new_event_n =
                            continuation_token.event_n as usize + filtered_events.len();

                        all_events.extend(filtered_events);

                        if all_events.len() >= chunk_size as usize {
                            // check if there are still more events that we haven't fetched yet. if
                            // yes, we need to return a continuation
                            // token that points to the next event to
                            // start fetching from.
                            let token = if continuation_token.txn_n < txs.len() as u64 - 1
                                || new_event_n < txn_events_len
                            {
                                continuation_token.event_n = new_event_n as u64;
                                Some(continuation_token.to_string())
                            } else {
                                None
                            };

                            return Ok(EventsPage {
                                events: all_events,
                                continuation_token: token,
                            });
                        }

                        continuation_token.txn_n += 1;
                        continuation_token.event_n = 0;
                    }
                }
            }

            if is_from_pending && is_to_pending {
                return Ok(EventsPage { events: all_events, continuation_token: None });
            }
        }

        Ok(EventsPage { events: all_events, continuation_token: None })
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

#[derive(Debug)]
struct FilteredEvents<'a, I: Iterator<Item = &'a Event>> {
    iter: I,
    address: Option<ContractAddress>,
    keys: Option<Vec<Vec<FieldElement>>>,
}

impl<'a, I: Iterator<Item = &'a Event>> Iterator for FilteredEvents<'a, I> {
    type Item = &'a Event;

    fn next(&mut self) -> Option<Self::Item> {
        for event in self.iter.by_ref() {
            // Check if the event matches the address filter
            if !self.address.map_or(true, |addr| addr == event.from_address) {
                continue;
            }

            // Check if the event matches the keys filter
            let matched = match &self.keys {
                None => true,
                Some(filters) => filters.iter().enumerate().all(|(i, keys)| {
                    event.keys.len() > i && (keys.is_empty() || keys.contains(&event.keys[i]))
                }),
            };

            if matched {
                return Some(event);
            }
        }

        None
    }
}

fn filter_events<'a, I: Iterator<Item = &'a Event>>(
    events: I,
    address: Option<ContractAddress>,
    filter_keys: Option<Vec<Vec<FieldElement>>>,
) -> FilteredEvents<'a, I> {
    FilteredEvents { iter: events, address, keys: filter_keys }
}
