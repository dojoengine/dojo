//! Server implementation for the Starknet JSON-RPC API.

mod read;
mod trace;
mod write;

use std::sync::Arc;

use katana_core::{
    backend::{contract::StarknetContract, Backend},
    pool::TransactionPool,
    service::block_producer::{BlockProducer, BlockProducerMode, PendingExecutor},
};
use katana_executor::ExecutorFactory;
use katana_primitives::{
    block::{BlockHash, BlockHashOrNumber, BlockIdOrTag, BlockNumber, BlockTag},
    class::CompiledClass,
    contract::{Nonce, StorageKey, StorageValue},
    env::BlockEnv,
    transaction::{TxHash, TxWithHash},
};
use katana_primitives::{
    class::ClassHash, contract::ContractAddress, transaction::ExecutableTxWithHash,
};
use katana_provider::traits::{
    block::{BlockHashProvider, BlockNumberProvider},
    env::BlockEnvProvider,
    state::{StateFactoryProvider, StateProvider},
    transaction::TransactionProvider,
};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::FeeEstimate;
use katana_tasks::{BlockingTaskPool, TokioTaskSpawner};

#[allow(missing_debug_implementations)]
pub struct StarknetApi<EF: ExecutorFactory> {
    pool: Arc<TransactionPool>,
    backend: Arc<Backend<EF>>,
    block_producer: Arc<BlockProducer<EF>>,
    blocking_task_pool: BlockingTaskPool,
}

impl<EF: ExecutorFactory> Clone for StarknetApi<EF> {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            backend: Arc::clone(&self.backend),
            block_producer: Arc::clone(&self.block_producer),
            blocking_task_pool: self.blocking_task_pool.clone(),
        }
    }
}

impl<EF: ExecutorFactory> StarknetApi<EF> {
    pub fn new(
        pool: Arc<TransactionPool>,
        backend: Arc<Backend<EF>>,
        block_producer: Arc<BlockProducer<EF>>,
    ) -> Self {
        let blocking_task_pool =
            BlockingTaskPool::new().expect("failed to create blocking task pool");

        Self { pool, backend, block_producer, blocking_task_pool }
    }

    async fn on_cpu_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        self.blocking_task_pool.spawn(move || func(this)).await.unwrap()
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
        let executor = self.backend.executor_factory.with_state_and_block_env(state, env);
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
        match &*self.block_producer.inner.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.executor()),
        }
    }

    fn state(&self, block_id: &BlockIdOrTag) -> Result<Box<dyn StateProvider>, StarknetApiError> {
        let provider = self.backend.blockchain.provider();

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
        let provider = self.backend.blockchain.provider();

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

    pub fn block_producer(&self) -> &BlockProducer<EF> {
        &self.block_producer
    }

    pub fn backend(&self) -> &Backend<EF> {
        &self.backend
    }

    pub fn add_transaction_to_pool(&self, tx: ExecutableTxWithHash) {
        self.pool.add_transaction(tx);
    }

    pub fn block_hash_and_number(&self) -> Result<(BlockHash, BlockNumber), StarknetApiError> {
        let provider = self.backend.blockchain.provider();
        let hash = provider.latest_hash()?;
        let number = provider.latest_number()?;
        Ok((hash, number))
    }

    pub fn class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, StarknetApiError> {
        let state = self.state(&block_id)?;
        let class_hash = state.class_hash_of_contract(contract_address)?;
        class_hash.ok_or(StarknetApiError::ContractNotFound)
    }

    pub fn class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: ClassHash,
    ) -> Result<StarknetContract, StarknetApiError> {
        let state = self.state(&block_id)?;

        let Some(class) = state.class(class_hash)? else {
            return Err(StarknetApiError::ClassHashNotFound);
        };

        match class {
            CompiledClass::Deprecated(class) => Ok(StarknetContract::Legacy(class)),
            CompiledClass::Class(_) => match state.sierra_class(class_hash)? {
                Some(sierra_class) => Ok(StarknetContract::Sierra(sierra_class)),
                None => Err(StarknetApiError::UnexpectedError {
                    reason: "Class hash exist, but missing its Sierra class".to_string(),
                }),
            },
        }
    }

    pub fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockIdOrTag,
    ) -> Result<StorageValue, StarknetApiError> {
        let state = self.state(&block_id)?;

        // check that contract exist by checking the class hash of the contract
        let Some(_) = StateProvider::class_hash_of_contract(&state, contract_address)? else {
            return Err(SequencerError::ContractNotFound(contract_address));
        };

        let value = StateProvider::storage(&state, contract_address, storage_key)?;
        Ok(value.unwrap_or_default())
    }

    pub fn block_number(&self) -> Result<BlockNumber, StarknetApiError> {
        let num = BlockNumberProvider::latest_number(&self.backend.blockchain.provider())?;
        Ok(num)
    }

    pub fn block_tx_count(&self, block_id: BlockIdOrTag) -> SequencerResult<Option<u64>> {
        let provider = self.backend.blockchain.provider();

        let count = match block_id {
            BlockIdOrTag::Tag(BlockTag::Pending) => match self.pending_executor() {
                Some(exec) => Some(exec.read().transactions().len() as u64),

                None => {
                    let hash = BlockHashProvider::latest_hash(provider)?;
                    TransactionProvider::transaction_count_by_block(provider, hash.into())?
                }
            },

            BlockIdOrTag::Tag(BlockTag::Latest) => {
                let num = BlockNumberProvider::latest_number(provider)?;
                TransactionProvider::transaction_count_by_block(provider, num.into())?
            }

            BlockIdOrTag::Number(num) => {
                TransactionProvider::transaction_count_by_block(provider, num.into())?
            }

            BlockIdOrTag::Hash(hash) => {
                TransactionProvider::transaction_count_by_block(provider, hash.into())?
            }
        };

        Ok(count)
    }

    pub fn nonce_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> Result<Nonce, StarknetApiError> {
        let state = self.state(&block_id)?;
        let nonce = state.nonce(contract_address)?.ok_or(StarknetApiError::ContractNotFound)?;
        Ok(nonce)
    }

    pub fn transaction(&self, hash: &TxHash) -> Result<TxWithHash, StarknetApiError> {
        let tx = self.backend.blockchain.provider().transaction_by_hash(*hash)?;

        let tx = match tx {
            tx @ Some(_) => tx,
            None => {
                // check if the transaction is in the pending block
                self.pending_executor().as_ref().and_then(|exec| {
                    exec.read()
                        .transactions()
                        .iter()
                        .find(|(tx, _)| tx.hash == *hash)
                        .map(|(tx, _)| tx.clone())
                })
            }
        };

        tx.ok_or(StarknetApiError::TxnHashNotFound)
    }

    pub fn events(
        &self,
        from_block: BlockIdOrTag,
        to_block: BlockIdOrTag,
        address: Option<ContractAddress>,
        keys: Option<Vec<Vec<FieldElement>>>,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> SequencerResult<EventsPage> {
        let provider = self.backend.blockchain.provider();
        let mut current_block = 0;

        let (mut from_block, to_block) = {
            let from = BlockIdReader::convert_block_id(provider, from_block)?
                .ok_or(SequencerError::BlockNotFound(to_block))?;
            let to = BlockIdReader::convert_block_id(provider, to_block)?
                .ok_or(SequencerError::BlockNotFound(to_block))?;
            (from, to)
        };

        let mut continuation_token = match continuation_token {
            Some(token) => ContinuationToken::parse(token)?,
            None => ContinuationToken::default(),
        };

        // skip blocks that have been already read
        from_block += continuation_token.block_n;

        let mut filtered_events = Vec::with_capacity(chunk_size as usize);

        for i in from_block..=to_block {
            let block_hash = BlockHashProvider::block_hash_by_num(provider, i)?
                .ok_or(SequencerError::BlockNotFound(BlockIdOrTag::Number(i)))?;

            let receipts = ReceiptProvider::receipts_by_block(provider, BlockHashOrNumber::Num(i))?
                .ok_or(SequencerError::BlockNotFound(BlockIdOrTag::Number(i)))?;

            let tx_range = BlockProvider::block_body_indices(provider, BlockHashOrNumber::Num(i))?
                .ok_or(SequencerError::BlockNotFound(BlockIdOrTag::Number(i)))?;
            let tx_hashes =
                TransactionsProviderExt::transaction_hashes_in_range(provider, tx_range.into())?;

            let txn_n = receipts.len();
            if (txn_n as u64) < continuation_token.txn_n {
                return Err(SequencerError::ContinuationToken(
                    ContinuationTokenError::InvalidToken,
                ));
            }

            for (tx_hash, events) in tx_hashes
                .into_iter()
                .zip(receipts.iter().map(|r| r.events()))
                .skip(continuation_token.txn_n as usize)
            {
                let txn_events_len: usize = events.len();

                // check if continuation_token.event_n is correct
                match (txn_events_len as u64).cmp(&continuation_token.event_n) {
                    Ordering::Greater => (),
                    Ordering::Less => {
                        return Err(SequencerError::ContinuationToken(
                            ContinuationTokenError::InvalidToken,
                        ));
                    }
                    Ordering::Equal => {
                        continuation_token.txn_n += 1;
                        continuation_token.event_n = 0;
                        continue;
                    }
                }

                // skip events
                let txn_events = events.iter().skip(continuation_token.event_n as usize);

                let (new_filtered_events, continuation_index) = filter_events_by_params(
                    txn_events,
                    address,
                    keys.clone(),
                    Some((chunk_size as usize) - filtered_events.len()),
                );

                filtered_events.extend(new_filtered_events.iter().map(|e| EmittedEvent {
                    from_address: e.from_address.into(),
                    keys: e.keys.clone(),
                    data: e.data.clone(),
                    block_hash: Some(block_hash),
                    block_number: Some(i),
                    transaction_hash: tx_hash,
                }));

                if filtered_events.len() >= chunk_size as usize {
                    let token = if current_block < to_block
                        || continuation_token.txn_n < txn_n as u64 - 1
                        || continuation_index < txn_events_len
                    {
                        continuation_token.event_n = continuation_index as u64;
                        Some(continuation_token.to_string())
                    } else {
                        None
                    };
                    return Ok(EventsPage { events: filtered_events, continuation_token: token });
                }

                continuation_token.txn_n += 1;
                continuation_token.event_n = 0;
            }

            current_block += 1;
            continuation_token.block_n += 1;
            continuation_token.txn_n = 0;
        }

        Ok(EventsPage { events: filtered_events, continuation_token: None })
    }

    pub fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), SequencerError> {
        if self.has_pending_transactions() {
            return Err(SequencerError::PendingTransactions);
        }
        self.backend().block_context_generator.write().next_block_start_time = timestamp;
        Ok(())
    }

    pub fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), SequencerError> {
        if self.has_pending_transactions() {
            return Err(SequencerError::PendingTransactions);
        }
        self.backend().block_context_generator.write().block_timestamp_offset += timestamp as i64;
        Ok(())
    }

    pub fn has_pending_transactions(&self) -> bool {
        if let Some(ref exec) = self.pending_executor() {
            !exec.read().transactions().is_empty()
        } else {
            false
        }
    }
}

fn filter_events_by_params(
    events: Skip<Iter<'_, Event>>,
    address: Option<ContractAddress>,
    filter_keys: Option<Vec<Vec<FieldElement>>>,
    max_results: Option<usize>,
) -> (Vec<Event>, usize) {
    let mut filtered_events = vec![];
    let mut index = 0;

    // Iterate on block events.
    for event in events {
        index += 1;
        if !address.map_or(true, |addr| addr == event.from_address) {
            continue;
        }

        let match_keys = match filter_keys {
            // From starknet-api spec:
            // Per key (by position), designate the possible values to be matched for events to be
            // returned. Empty array designates 'any' value"
            Some(ref filter_keys) => filter_keys.iter().enumerate().all(|(i, keys)| {
                // Lets say we want to filter events which are either named `Event1` or `Event2` and
                // custom key `0x1` or `0x2` Filter: [[sn_keccack("Event1"),
                // sn_keccack("Event2")], ["0x1", "0x2"]]

                // This checks: number of keys in event >= number of keys in filter (we check > i
                // and not >= i because i is zero indexed) because otherwise this
                // event doesn't contain all the keys we requested
                event.keys.len() > i &&
                    // This checks: Empty array desginates 'any' value
                    (keys.is_empty()
                    ||
                    // This checks: If this events i'th value is one of the requested value in filter_keys[i]
                    keys.contains(&event.keys[i]))
            }),
            None => true,
        };

        if match_keys {
            filtered_events.push(event.clone());
            if let Some(max_results) = max_results {
                if filtered_events.len() >= max_results {
                    break;
                }
            }
        }
    }
    (filtered_events, index)
}
