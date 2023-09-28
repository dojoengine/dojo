use std::cmp::Ordering;
use std::iter::Skip;
use std::slice::Iter;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use auto_impl::auto_impl;
use blockifier::execution::contract_class::ContractClass;
use blockifier::state::state_api::{State, StateReader};
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, Event, EventsPage, FeeEstimate, FieldElement,
    MaybePendingTransactionReceipt, StateUpdate,
};
use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use crate::backend::config::StarknetConfig;
use crate::backend::contract::StarknetContract;
use crate::backend::storage::block::{ExecutedBlock, PartialBlock, PartialHeader};
use crate::backend::storage::transaction::{
    DeclareTransaction, DeployAccountTransaction, InvokeTransaction, KnownTransaction,
    PendingTransaction, Transaction,
};
use crate::backend::{Backend, ExternalFunctionCall};
use crate::db::{AsStateRefDb, StateExtRef, StateRefDb};
use crate::execution::{MaybeInvalidExecutedTransaction, PendingState};
use crate::messaging::{MessageService, MessagingConfig};
use crate::pool::TransactionPool;
use crate::sequencer_error::SequencerError;
use crate::service::{BlockProducer, BlockProducerMode, NodeService, TransactionMiner};
use crate::utils::event::{ContinuationToken, ContinuationTokenError};

type SequencerResult<T> = Result<T, SequencerError>;

#[derive(Debug, Default)]
pub struct SequencerConfig {
    pub block_time: Option<u64>,
    pub no_mining: bool,
    pub messaging: Option<MessagingConfig>,
}

#[async_trait]
#[auto_impl(Arc)]
pub trait Sequencer {
    fn block_producer(&self) -> &BlockProducer;

    fn backend(&self) -> &Backend;

    async fn state(&self, block_id: &BlockId) -> SequencerResult<StateRefDb>;

    async fn chain_id(&self) -> ChainId;

    async fn transaction_receipt(
        &self,
        hash: &FieldElement,
    ) -> Option<MaybePendingTransactionReceipt>;

    async fn nonce_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<Nonce>;

    async fn block_number(&self) -> u64;

    async fn block(&self, block_id: BlockId) -> Option<ExecutedBlock>;

    async fn transaction(&self, hash: &FieldElement) -> Option<KnownTransaction>;

    async fn class_hash_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<ClassHash>;

    async fn class(
        &self,
        block_id: BlockId,
        class_hash: ClassHash,
    ) -> SequencerResult<StarknetContract>;

    async fn block_hash_and_number(&self) -> (FieldElement, u64);

    async fn call(
        &self,
        block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> SequencerResult<Vec<StarkFelt>>;

    async fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockId,
    ) -> SequencerResult<StarkFelt>;

    async fn add_deploy_account_transaction(
        &self,
        transaction: DeployAccountTransaction,
    ) -> (FieldElement, FieldElement);

    fn add_declare_transaction(&self, transaction: DeclareTransaction);

    fn add_invoke_transaction(&self, transaction: InvokeTransaction);

    async fn estimate_fee(
        &self,
        transactions: Vec<Transaction>,
        block_id: BlockId,
    ) -> SequencerResult<Vec<FeeEstimate>>;

    async fn events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
        address: Option<FieldElement>,
        keys: Option<Vec<Vec<FieldElement>>>,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> SequencerResult<EventsPage>;

    async fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate>;

    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), SequencerError>;

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), SequencerError>;

    async fn has_pending_transactions(&self) -> bool;

    async fn set_storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        value: StarkFelt,
    ) -> Result<(), SequencerError>;
}

pub struct KatanaSequencer {
    pub config: SequencerConfig,
    pub pool: Arc<TransactionPool>,
    pub backend: Arc<Backend>,
    pub block_producer: BlockProducer,
}

impl KatanaSequencer {
    pub async fn new(config: SequencerConfig, starknet_config: StarknetConfig) -> Self {
        let backend = Arc::new(Backend::new(starknet_config).await);

        let pool = Arc::new(TransactionPool::new());
        let miner = TransactionMiner::new(pool.add_listener());

        let block_producer = if let Some(block_time) = config.block_time {
            BlockProducer::interval(
                Arc::clone(&backend),
                backend.state.read().await.as_ref_db(),
                block_time,
            )
        } else if config.no_mining {
            BlockProducer::on_demand(Arc::clone(&backend), backend.state.read().await.as_ref_db())
        } else {
            BlockProducer::instant(Arc::clone(&backend))
        };

        tokio::spawn(NodeService::new(Arc::clone(&pool), miner, block_producer.clone()));

        if let Some(config) = config.messaging.clone() {
            tokio::spawn(
                MessageService::new(config.clone(), Arc::clone(&backend), Arc::clone(&pool)).await,
            );
        }

        Self { pool, config, backend, block_producer }
    }

    /// Returns the pending state if the sequencer is running in _interval_ mode. Otherwise `None`.
    pub fn pending_state(&self) -> Option<Arc<PendingState>> {
        match &*self.block_producer.inner.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.state()),
        }
    }

    async fn verify_contract_exists(&self, contract_address: &ContractAddress) -> bool {
        self.backend
            .state
            .write()
            .await
            .get_class_hash_at(*contract_address)
            .is_ok_and(|c| c != ClassHash::default())
    }
}

#[async_trait]
impl Sequencer for KatanaSequencer {
    fn block_producer(&self) -> &BlockProducer {
        &self.block_producer
    }

    fn backend(&self) -> &Backend {
        &self.backend
    }

    async fn state(&self, block_id: &BlockId) -> SequencerResult<StateRefDb> {
        match block_id {
            BlockId::Tag(BlockTag::Latest) => Ok(self.backend.state.read().await.as_ref_db()),

            BlockId::Tag(BlockTag::Pending) => {
                if let Some(state) = self.pending_state() {
                    Ok(state.state.read().as_ref_db())
                } else {
                    Ok(self.backend.state.read().await.as_ref_db())
                }
            }

            _ => {
                if let Some(hash) = self.backend.blockchain.block_hash(*block_id) {
                    self.backend
                        .states
                        .read()
                        .await
                        .get(&hash)
                        .cloned()
                        .ok_or(SequencerError::StateNotFound(*block_id))
                } else {
                    Err(SequencerError::BlockNotFound(*block_id))
                }
            }
        }
    }

    async fn add_deploy_account_transaction(
        &self,
        transaction: DeployAccountTransaction,
    ) -> (FieldElement, FieldElement) {
        let transaction_hash = transaction.inner.transaction_hash.0.into();
        let contract_address = transaction.contract_address;

        self.pool.add_transaction(Transaction::DeployAccount(transaction));

        (transaction_hash, contract_address)
    }

    fn add_declare_transaction(&self, transaction: DeclareTransaction) {
        self.pool.add_transaction(Transaction::Declare(transaction))
    }

    fn add_invoke_transaction(&self, transaction: InvokeTransaction) {
        self.pool.add_transaction(Transaction::Invoke(transaction))
    }

    async fn estimate_fee(
        &self,
        transactions: Vec<Transaction>,
        block_id: BlockId,
    ) -> SequencerResult<Vec<FeeEstimate>> {
        let state = self.state(&block_id).await?;
        self.backend.estimate_fee(transactions, state).map_err(SequencerError::TransactionExecution)
    }

    async fn block_hash_and_number(&self) -> (FieldElement, u64) {
        let hash = self.backend.blockchain.storage.read().latest_hash;
        let number = self.backend.blockchain.storage.read().latest_number;
        (hash, number)
    }

    async fn class_hash_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<ClassHash> {
        if !self.verify_contract_exists(&contract_address).await {
            return Err(SequencerError::ContractNotFound(contract_address));
        }

        let mut state = self.state(&block_id).await?;
        state.get_class_hash_at(contract_address).map_err(SequencerError::State)
    }

    async fn class(
        &self,
        block_id: BlockId,
        class_hash: ClassHash,
    ) -> SequencerResult<StarknetContract> {
        let mut state = self.state(&block_id).await?;

        if let ContractClass::V0(c) =
            state.get_compiled_contract_class(&class_hash).map_err(SequencerError::State)?
        {
            Ok(StarknetContract::Legacy(c))
        } else {
            state
                .get_sierra_class(&class_hash)
                .map(StarknetContract::Sierra)
                .map_err(SequencerError::State)
        }
    }

    async fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockId,
    ) -> SequencerResult<StarkFelt> {
        if !self.verify_contract_exists(&contract_address).await {
            return Err(SequencerError::ContractNotFound(contract_address));
        }

        let mut state = self.state(&block_id).await?;
        state.get_storage_at(contract_address, storage_key).map_err(SequencerError::State)
    }

    async fn chain_id(&self) -> ChainId {
        self.backend.env.read().block.chain_id.clone()
    }

    async fn block_number(&self) -> u64 {
        self.backend.blockchain.storage.read().latest_number
    }

    async fn block(&self, block_id: BlockId) -> Option<ExecutedBlock> {
        let block_id = match block_id {
            BlockId::Tag(BlockTag::Pending) if self.block_producer.is_instant_mining() => {
                BlockId::Tag(BlockTag::Latest)
            }
            _ => block_id,
        };

        match block_id {
            BlockId::Tag(BlockTag::Pending) => {
                let state = self.pending_state().expect("pending state should exist");

                let block_context = self.backend.env.read().block.clone();
                let latest_hash = self.backend.blockchain.storage.read().latest_hash;

                let header = PartialHeader {
                    parent_hash: latest_hash,
                    gas_price: block_context.gas_price,
                    number: block_context.block_number.0,
                    timestamp: block_context.block_timestamp.0,
                    sequencer_address: (*block_context.sequencer_address.0.key()).into(),
                };

                let (transactions, outputs) = {
                    state
                        .executed_transactions
                        .read()
                        .iter()
                        .filter_map(|tx| match tx {
                            MaybeInvalidExecutedTransaction::Valid(tx) => {
                                Some((tx.clone(), tx.output.clone()))
                            }
                            _ => None,
                        })
                        .unzip()
                };

                Some(ExecutedBlock::Pending(PartialBlock { header, transactions, outputs }))
            }

            _ => {
                let hash = self.backend.blockchain.block_hash(block_id)?;
                self.backend.blockchain.storage.read().blocks.get(&hash).map(|b| b.clone().into())
            }
        }
    }

    async fn nonce_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<Nonce> {
        if !self.verify_contract_exists(&contract_address).await {
            return Err(SequencerError::ContractNotFound(contract_address));
        }

        let mut state = self.state(&block_id).await?;
        state.get_nonce_at(contract_address).map_err(SequencerError::State)
    }

    async fn call(
        &self,
        block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> SequencerResult<Vec<StarkFelt>> {
        if !self.verify_contract_exists(&function_call.contract_address).await {
            return Err(SequencerError::ContractNotFound(function_call.contract_address));
        }

        let state = self.state(&block_id).await?;

        self.backend
            .call(function_call, state)
            .map_err(SequencerError::EntryPointExecution)
            .map(|execution_info| execution_info.execution.retdata.0)
    }

    async fn transaction_receipt(
        &self,
        hash: &FieldElement,
    ) -> Option<MaybePendingTransactionReceipt> {
        let transaction = self.transaction(hash).await?;

        match transaction {
            KnownTransaction::Rejected(_) => None,
            KnownTransaction::Pending(tx) => {
                Some(MaybePendingTransactionReceipt::PendingReceipt(tx.receipt()))
            }
            KnownTransaction::Included(tx) => {
                Some(MaybePendingTransactionReceipt::Receipt(tx.receipt()))
            }
        }
    }

    async fn transaction(&self, hash: &FieldElement) -> Option<KnownTransaction> {
        let tx = self.backend.blockchain.storage.read().transactions.get(hash).cloned();
        match tx {
            Some(tx) => Some(tx),
            // If the requested transaction is not available in the storage then
            // check if it is available in the pending block.
            None => self.pending_state().as_ref().and_then(|state| {
                state.executed_transactions.read().iter().find_map(|tx| match tx {
                    MaybeInvalidExecutedTransaction::Valid(tx) if tx.inner.hash() == *hash => {
                        Some(PendingTransaction(tx.clone()).into())
                    }
                    MaybeInvalidExecutedTransaction::Invalid(tx) if tx.inner.hash() == *hash => {
                        Some(tx.as_ref().clone().into())
                    }
                    _ => None,
                })
            }),
        }
    }

    async fn events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
        address: Option<FieldElement>,
        keys: Option<Vec<Vec<FieldElement>>>,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> SequencerResult<EventsPage> {
        let mut current_block = 0;

        let (mut from_block, to_block) = {
            let storage = &self.backend.blockchain;

            let from = storage
                .block_hash(from_block)
                .and_then(|hash| storage.storage.read().blocks.get(&hash).map(|b| b.header.number))
                .ok_or(SequencerError::BlockNotFound(from_block))?;

            let to = storage
                .block_hash(to_block)
                .and_then(|hash| storage.storage.read().blocks.get(&hash).map(|b| b.header.number))
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
            let block = self
                .backend
                .blockchain
                .storage
                .read()
                .block_by_number(i)
                .cloned()
                .ok_or(SequencerError::BlockNotFound(BlockId::Number(i)))?;

            // to get the current block hash we need to get the parent hash of the next block
            // if the current block is the latest block then we use the latest hash
            let block_hash = self
                .backend
                .blockchain
                .storage
                .read()
                .block_by_number(i + 1)
                .map(|b| b.header.parent_hash)
                .unwrap_or(self.backend.blockchain.storage.read().latest_hash);

            let block_number = i;

            let txn_n = block.transactions.len();
            if (txn_n as u64) < continuation_token.txn_n {
                return Err(SequencerError::ContinuationToken(
                    ContinuationTokenError::InvalidToken,
                ));
            }

            for (txn_output, txn) in
                block.outputs.iter().zip(block.transactions).skip(continuation_token.txn_n as usize)
            {
                let txn_events_len: usize = txn_output.events.len();

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
                let txn_events = txn_output.events.iter().skip(continuation_token.event_n as usize);

                let (new_filtered_events, continuation_index) = filter_events_by_params(
                    txn_events,
                    address,
                    keys.clone(),
                    Some((chunk_size as usize) - filtered_events.len()),
                );

                filtered_events.extend(new_filtered_events.iter().map(|e| EmittedEvent {
                    from_address: e.from_address,
                    keys: e.keys.clone(),
                    data: e.data.clone(),
                    block_hash,
                    block_number,
                    transaction_hash: txn.inner.hash(),
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

    async fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate> {
        let block_number = self
            .backend
            .blockchain
            .block_hash(block_id)
            .ok_or(SequencerError::BlockNotFound(block_id))?;

        self.backend
            .blockchain
            .storage
            .read()
            .state_update
            .get(&block_number)
            .cloned()
            .ok_or(SequencerError::StateUpdateNotFound(block_id))
    }

    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), SequencerError> {
        if self.has_pending_transactions().await {
            return Err(SequencerError::PendingTransactions);
        }
        self.backend().block_context_generator.write().next_block_start_time = timestamp;
        Ok(())
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), SequencerError> {
        if self.has_pending_transactions().await {
            return Err(SequencerError::PendingTransactions);
        }
        self.backend().block_context_generator.write().block_timestamp_offset += timestamp as i64;
        Ok(())
    }

    async fn has_pending_transactions(&self) -> bool {
        if let Some(ref pending) = self.pending_state() {
            !pending.executed_transactions.read().is_empty()
        } else {
            false
        }
    }

    async fn set_storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        value: StarkFelt,
    ) -> Result<(), SequencerError> {
        if let Some(ref pending) = self.pending_state() {
            pending.state.write().set_storage_at(contract_address, storage_key, value);
        } else {
            self.backend().state.write().await.set_storage_at(contract_address, storage_key, value);
        }
        Ok(())
    }
}

fn filter_events_by_params(
    events: Skip<Iter<'_, Event>>,
    address: Option<FieldElement>,
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
