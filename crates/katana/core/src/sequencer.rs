use std::cmp::Ordering;
use std::iter::Skip;
use std::slice::Iter;
use std::sync::Arc;

use anyhow::Result;
use blockifier::block_context::BlockContext;
use blockifier::execution::errors::{EntryPointExecutionError, PreExecutionError};
use blockifier::transaction::errors::TransactionExecutionError;
use katana_executor::blockifier::state::StateRefDb;
use katana_executor::blockifier::utils::{block_context_from_envs, EntryPointCall};
use katana_executor::blockifier::PendingState;
use katana_primitives::block::{BlockHash, BlockHashOrNumber, BlockIdOrTag, BlockNumber};
use katana_primitives::chain::ChainId;
use katana_primitives::class::{ClassHash, CompiledClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::event::{ContinuationToken, ContinuationTokenError};
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
    ReceiptProvider, TransactionProvider, TransactionsProviderExt,
};
use starknet::core::types::{BlockTag, EmittedEvent, EventsPage, FeeEstimate};

use crate::backend::config::StarknetConfig;
use crate::backend::contract::StarknetContract;
use crate::backend::Backend;
use crate::pool::TransactionPool;
use crate::sequencer_error::SequencerError;
use crate::service::block_producer::{BlockProducer, BlockProducerMode};
#[cfg(feature = "messaging")]
use crate::service::messaging::MessagingConfig;
#[cfg(feature = "messaging")]
use crate::service::messaging::MessagingService;
use crate::service::{NodeService, TransactionMiner};

type SequencerResult<T> = Result<T, SequencerError>;

#[derive(Debug, Default)]
pub struct SequencerConfig {
    pub block_time: Option<u64>,
    pub no_mining: bool,
    #[cfg(feature = "messaging")]
    pub messaging: Option<MessagingConfig>,
}

pub struct KatanaSequencer {
    pub config: SequencerConfig,
    pub pool: Arc<TransactionPool>,
    pub backend: Arc<Backend>,
    pub block_producer: BlockProducer,
}

impl KatanaSequencer {
    pub async fn new(
        config: SequencerConfig,
        starknet_config: StarknetConfig,
    ) -> anyhow::Result<Self> {
        let backend = Arc::new(Backend::new(starknet_config).await);

        let pool = Arc::new(TransactionPool::new());
        let miner = TransactionMiner::new(pool.add_listener());

        let state = StateFactoryProvider::latest(backend.blockchain.provider())
            .map(StateRefDb::new)
            .unwrap();

        let block_producer = if config.block_time.is_some() || config.no_mining {
            let block_num = backend.blockchain.provider().latest_number()?;

            let mut block_env =
                backend.blockchain.provider().block_env_at(block_num.into())?.unwrap();
            backend.update_block_env(&mut block_env);
            let cfg_env = backend.chain_cfg_env();

            if let Some(interval) = config.block_time {
                BlockProducer::interval(Arc::clone(&backend), state, interval, (block_env, cfg_env))
            } else {
                BlockProducer::on_demand(Arc::clone(&backend), state, (block_env, cfg_env))
            }
        } else {
            BlockProducer::instant(Arc::clone(&backend))
        };

        #[cfg(feature = "messaging")]
        let messaging = if let Some(config) = config.messaging.clone() {
            MessagingService::new(config, Arc::clone(&pool), Arc::clone(&backend)).await.ok()
        } else {
            None
        };

        tokio::spawn(NodeService {
            miner,
            pool: Arc::clone(&pool),
            block_producer: block_producer.clone(),
            #[cfg(feature = "messaging")]
            messaging,
        });

        Ok(Self { pool, config, backend, block_producer })
    }

    /// Returns the pending state if the sequencer is running in _interval_ mode. Otherwise `None`.
    pub fn pending_state(&self) -> Option<Arc<PendingState>> {
        match &*self.block_producer.inner.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.state()),
        }
    }

    pub fn block_producer(&self) -> &BlockProducer {
        &self.block_producer
    }

    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    pub fn block_execution_context_at(
        &self,
        block_id: BlockIdOrTag,
    ) -> SequencerResult<Option<BlockContext>> {
        let provider = self.backend.blockchain.provider();
        let cfg_env = self.backend().chain_cfg_env();

        if let BlockIdOrTag::Tag(BlockTag::Pending) = block_id {
            if let Some(state) = self.pending_state() {
                let (block_env, _) = state.block_execution_envs();
                return Ok(Some(block_context_from_envs(&block_env, &cfg_env)));
            }
        }

        let block_num = match block_id {
            BlockIdOrTag::Tag(BlockTag::Pending) | BlockIdOrTag::Tag(BlockTag::Latest) => {
                provider.latest_number()?
            }

            BlockIdOrTag::Hash(hash) => provider
                .block_number_by_hash(hash)?
                .ok_or(SequencerError::BlockNotFound(block_id))?,

            BlockIdOrTag::Number(num) => num,
        };

        provider
            .block_env_at(block_num.into())?
            .map(|block_env| Some(block_context_from_envs(&block_env, &cfg_env)))
            .ok_or(SequencerError::BlockNotFound(block_id))
    }

    pub fn state(&self, block_id: &BlockIdOrTag) -> SequencerResult<Box<dyn StateProvider>> {
        let provider = self.backend.blockchain.provider();

        match block_id {
            BlockIdOrTag::Tag(BlockTag::Latest) => {
                let state = StateFactoryProvider::latest(provider)?;
                Ok(state)
            }

            BlockIdOrTag::Tag(BlockTag::Pending) => {
                if let Some(state) = self.pending_state() {
                    Ok(Box::new(state.state.clone()))
                } else {
                    let state = StateFactoryProvider::latest(provider)?;
                    Ok(state)
                }
            }

            BlockIdOrTag::Hash(hash) => {
                StateFactoryProvider::historical(provider, BlockHashOrNumber::Hash(*hash))?
                    .ok_or(SequencerError::BlockNotFound(*block_id))
            }

            BlockIdOrTag::Number(num) => {
                StateFactoryProvider::historical(provider, BlockHashOrNumber::Num(*num))?
                    .ok_or(SequencerError::BlockNotFound(*block_id))
            }
        }
    }

    pub fn add_transaction_to_pool(&self, tx: ExecutableTxWithHash) {
        self.pool.add_transaction(tx);
    }

    pub fn estimate_fee(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        block_id: BlockIdOrTag,
    ) -> SequencerResult<Vec<FeeEstimate>> {
        let state = self.state(&block_id)?;

        let block_context = self
            .block_execution_context_at(block_id)?
            .ok_or_else(|| SequencerError::BlockNotFound(block_id))?;

        katana_executor::blockifier::utils::estimate_fee(
            transactions.into_iter(),
            block_context,
            state,
            !self.backend.config.disable_validate,
        )
        .map_err(SequencerError::TransactionExecution)
    }

    pub fn block_hash_and_number(&self) -> SequencerResult<(BlockHash, BlockNumber)> {
        let provider = self.backend.blockchain.provider();
        let hash = BlockHashProvider::latest_hash(provider)?;
        let number = BlockNumberProvider::latest_number(provider)?;
        Ok((hash, number))
    }

    pub fn class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> SequencerResult<Option<ClassHash>> {
        let state = self.state(&block_id)?;
        let class_hash = StateProvider::class_hash_of_contract(&state, contract_address)?;
        Ok(class_hash)
    }

    pub fn class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: ClassHash,
    ) -> SequencerResult<Option<StarknetContract>> {
        let state = self.state(&block_id)?;

        let Some(class) = ContractClassProvider::class(&state, class_hash)? else {
            return Ok(None);
        };

        match class {
            CompiledClass::Deprecated(class) => Ok(Some(StarknetContract::Legacy(class))),
            CompiledClass::Class(_) => {
                let class = ContractClassProvider::sierra_class(&state, class_hash)?
                    .map(StarknetContract::Sierra);
                Ok(class)
            }
        }
    }

    pub fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockIdOrTag,
    ) -> SequencerResult<StorageValue> {
        let state = self.state(&block_id)?;

        // check that contract exist by checking the class hash of the contract
        let Some(_) = StateProvider::class_hash_of_contract(&state, contract_address)? else {
            return Err(SequencerError::ContractNotFound(contract_address));
        };

        let value = StateProvider::storage(&state, contract_address, storage_key)?;
        Ok(value.unwrap_or_default())
    }

    pub fn chain_id(&self) -> ChainId {
        self.backend.chain_id
    }

    pub fn block_number(&self) -> SequencerResult<BlockNumber> {
        let num = BlockNumberProvider::latest_number(&self.backend.blockchain.provider())?;
        Ok(num)
    }

    pub fn block_tx_count(&self, block_id: BlockIdOrTag) -> SequencerResult<Option<u64>> {
        let provider = self.backend.blockchain.provider();

        let count = match block_id {
            BlockIdOrTag::Tag(BlockTag::Pending) => match self.pending_state() {
                Some(state) => Some(state.executed_txs.read().len() as u64),
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
    ) -> SequencerResult<Option<Nonce>> {
        let state = self.state(&block_id)?;
        let nonce = StateProvider::nonce(&state, contract_address)?;
        Ok(nonce)
    }

    pub fn call(
        &self,
        request: EntryPointCall,
        block_id: BlockIdOrTag,
    ) -> SequencerResult<Vec<FieldElement>> {
        let state = self.state(&block_id)?;

        let block_context = self
            .block_execution_context_at(block_id)?
            .ok_or_else(|| SequencerError::BlockNotFound(block_id))?;

        let retdata = katana_executor::blockifier::utils::call(request, block_context, state)
            .map_err(|e| match e {
                TransactionExecutionError::ExecutionError(exe) => match exe {
                    EntryPointExecutionError::PreExecutionError(
                        PreExecutionError::UninitializedStorageAddress(addr),
                    ) => SequencerError::ContractNotFound(addr.into()),
                    _ => SequencerError::EntryPointExecution(exe),
                },
                _ => SequencerError::TransactionExecution(e),
            })?;

        Ok(retdata)
    }

    pub fn transaction(&self, hash: &TxHash) -> SequencerResult<Option<TxWithHash>> {
        let tx =
            TransactionProvider::transaction_by_hash(self.backend.blockchain.provider(), *hash)?;

        let tx @ Some(_) = tx else {
            return Ok(self.pending_state().as_ref().and_then(|state| {
                state
                    .executed_txs
                    .read()
                    .iter()
                    .find_map(|tx| if tx.0.hash == *hash { Some(tx.0.clone()) } else { None })
            }));
        };

        Ok(tx)
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
                    block_hash,
                    block_number: i,
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
        if let Some(ref pending) = self.pending_state() {
            !pending.executed_txs.read().is_empty()
        } else {
            false
        }
    }

    // pub async fn set_storage_at(
    //     &self,
    //     contract_address: ContractAddress,
    //     storage_key: StorageKey,
    //     value: StorageValue,
    // ) -> Result<(), SequencerError> { if let Some(ref pending) = self.pending_state() {
    //   StateWriter::set_storage(&pending.state, contract_address, storage_key, value)?; } Ok(())
    // }
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

#[cfg(test)]
mod tests {
    use katana_provider::traits::block::BlockNumberProvider;

    use super::{KatanaSequencer, SequencerConfig};
    use crate::backend::config::StarknetConfig;

    #[tokio::test]
    async fn init_interval_block_producer_with_correct_block_env() {
        let sequencer = KatanaSequencer::new(
            SequencerConfig { no_mining: true, ..Default::default() },
            StarknetConfig::default(),
        )
        .await
        .unwrap();

        let provider = sequencer.backend.blockchain.provider();

        let latest_num = provider.latest_number().unwrap();
        let producer_block_env = sequencer.pending_state().unwrap().block_execution_envs().0;

        assert_eq!(
            producer_block_env.number,
            latest_num + 1,
            "Pending block number should be latest block number + 1"
        );
    }
}
