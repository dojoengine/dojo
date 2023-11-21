use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use blockifier::execution::entry_point::{
    CallEntryPoint, CallInfo, EntryPointExecutionContext, ExecutionResources,
};
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::cached_state::{CachedState, MutRefState};
use blockifier::state::state_api::StateReader;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::AccountTransactionContext;
use flate2::write::GzEncoder;
use flate2::Compression;
use parking_lot::RwLock;
use starknet::core::types::{
    BlockId, BlockTag, FeeEstimate, MaybePendingBlockWithTxHashes, TransactionFinalityStatus,
};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::{ChainId, ContractAddress, EntryPointSelector, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::transaction::Calldata;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{info, trace, warn};

pub mod config;
pub mod contract;
pub mod in_memory_db;
pub mod storage;

use self::config::StarknetConfig;
use self::storage::block::{Block, PartialHeader};
use self::storage::transaction::{IncludedTransaction, Transaction};
use self::storage::{Blockchain, InMemoryBlockStates, Storage};
use crate::accounts::{Account, DevAccountGenerator};
use crate::backend::in_memory_db::MemDb;
use crate::backend::storage::transaction::KnownTransaction;
use crate::constants::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use crate::db::cached::CachedStateWrapper;
use crate::db::serde::state::SerializableState;
use crate::db::{Database, StateRefDb};
use crate::env::{BlockContextGenerator, Env};
use crate::execution::{ExecutionOutcome, MaybeInvalidExecutedTransaction, TransactionExecutor};
use crate::fork::db::ForkedDb;
use crate::sequencer_error::SequencerError;
use crate::service::block_producer::MinedBlockOutcome;
use crate::utils::{convert_state_diff_to_rpc_state_diff, get_current_timestamp};

pub struct ExternalFunctionCall {
    pub calldata: Calldata,
    pub contract_address: ContractAddress,
    pub entry_point_selector: EntryPointSelector,
}

pub struct Backend {
    /// The config used to generate the backend.
    pub config: RwLock<StarknetConfig>,
    /// stores all block related data in memory
    pub blockchain: Blockchain,
    /// Historic states of previous blocks
    pub states: AsyncRwLock<InMemoryBlockStates>,
    /// The chain environment values.
    pub env: Arc<RwLock<Env>>,
    pub block_context_generator: RwLock<BlockContextGenerator>,
    /// The latest state.
    pub state: Arc<AsyncRwLock<dyn Database>>,
    /// Prefunded dev accounts
    pub accounts: Vec<Account>,
}

impl Backend {
    pub async fn new(config: StarknetConfig) -> Self {
        let mut block_context = config.block_context();
        let block_context_generator = config.block_context_generator();

        let accounts = DevAccountGenerator::new(config.total_accounts)
            .with_seed(config.seed)
            .with_balance((*DEFAULT_PREFUNDED_ACCOUNT_BALANCE).into())
            .generate();

        let (state, storage): (Arc<AsyncRwLock<dyn Database>>, Arc<RwLock<Storage>>) =
            if let Some(forked_url) = config.fork_rpc_url.clone() {
                let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(forked_url.clone())));

                let forked_chain_id = provider.chain_id().await.unwrap();
                let forked_block_id = config
                    .fork_block_number
                    .map(BlockId::Number)
                    .unwrap_or(BlockId::Tag(BlockTag::Latest));

                let block = provider.get_block_with_tx_hashes(forked_block_id).await.unwrap();

                let MaybePendingBlockWithTxHashes::Block(block) = block else {
                    panic!("block to be forked is a pending block")
                };

                block_context.block_number = BlockNumber(block.block_number);
                block_context.block_timestamp = BlockTimestamp(block.timestamp);
                block_context.sequencer_address =
                    ContractAddress(patricia_key!(block.sequencer_address));
                block_context.chain_id =
                    ChainId(parse_cairo_short_string(&forked_chain_id).unwrap());

                let state = ForkedDb::new(Arc::clone(&provider), forked_block_id);

                trace!(
                    target: "backend",
                    "forking chain `{}` at block {} from {}",
                    parse_cairo_short_string(&forked_chain_id).unwrap(),
                    block.block_number,
                    forked_url
                );

                (
                    Arc::new(AsyncRwLock::new(state)),
                    Arc::new(RwLock::new(Storage::new_forked(
                        block.block_number,
                        block.block_hash,
                    ))),
                )
            } else {
                (
                    Arc::new(AsyncRwLock::new(MemDb::default())),
                    Arc::new(RwLock::new(Storage::new(&block_context))),
                )
            };

        for acc in &accounts {
            acc.deploy_and_fund(&mut *state.write().await)
                .expect("should be able to deploy and fund dev account");
        }

        if let Some(ref init_state) = config.init_state {
            state
                .write()
                .await
                .load_state(init_state.clone())
                .expect("failed to load initial state");
            info!(target: "backend", "Successfully loaded initial state");
        }

        let blockchain = Blockchain::new(storage);
        let states = InMemoryBlockStates::default();
        let env = Env { block: block_context };

        Self {
            state,
            env: Arc::new(RwLock::new(env)),
            config: RwLock::new(config),
            states: AsyncRwLock::new(states),
            blockchain,
            block_context_generator: RwLock::new(block_context_generator),
            accounts,
        }
    }

    /// Get the current state in a serializable format.
    pub async fn serialize_state(&self) -> Result<SerializableState, SequencerError> {
        self.state.read().await.dump_state().map_err(|_| SequencerError::StateSerialization)
    }

    pub async fn dump_state(&self) -> Result<Vec<u8>, SequencerError> {
        let serializable_state = self.serialize_state().await?;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&serde_json::to_vec(&serializable_state).unwrap_or_default())
            .map_err(|_| SequencerError::DataUnavailable)?;

        Ok(encoder.finish().unwrap_or_default())
    }

    pub fn estimate_fee(
        &self,
        transactions: Vec<Transaction>,
        state: StateRefDb,
    ) -> Result<Vec<FeeEstimate>, TransactionExecutionError> {
        let mut state = CachedStateWrapper::new(state);
        let block_context = self.env.read().block.clone();

        let mut estimations = Vec::with_capacity(transactions.len());

        let results = TransactionExecutor::new(&mut state, &block_context, false, transactions)
            .with_error_log()
            .execute();

        for res in results {
            let exec_info = res?;

            if exec_info.revert_error.is_some() {
                // TEMP: change this once `Reverted` transaction error is no longer `String`.
                return Err(TransactionExecutionError::ExecutionError(
                    EntryPointExecutionError::ExecutionFailed { error_data: vec![] },
                ));
            }

            let (l1_gas_usage, vm_resources) =
                extract_l1_gas_and_vm_usage(&exec_info.actual_resources);
            let l1_gas_by_vm_usage = calculate_l1_gas_by_vm_usage(&block_context, &vm_resources)?;
            let total_l1_gas_usage = l1_gas_usage as f64 + l1_gas_by_vm_usage;

            let gas_price = block_context.gas_price as u64;

            estimations.push(FeeEstimate {
                gas_consumed: total_l1_gas_usage.ceil() as u64,
                gas_price,
                overall_fee: total_l1_gas_usage.ceil() as u64 * gas_price,
            })
        }

        Ok(estimations)
    }

    /// Mines a new block based on the provided execution outcome.
    /// This method should only be called by the
    /// [IntervalBlockProducer](crate::service::block_producer::IntervalBlockProducer) when the node
    /// is running in `interval` mining mode.
    pub async fn mine_pending_block(
        &self,
        execution_outcome: ExecutionOutcome,
    ) -> (MinedBlockOutcome, StateRefDb) {
        let outcome = self.do_mine_block(execution_outcome).await;
        let new_state = self.state.read().await.as_ref_db();
        (outcome, new_state)
    }

    /// Updates the block context and mines an empty block.
    pub async fn mine_empty_block(&self) -> MinedBlockOutcome {
        self.update_block_context();
        self.do_mine_block(ExecutionOutcome::default()).await
    }

    pub async fn do_mine_block(&self, execution_outcome: ExecutionOutcome) -> MinedBlockOutcome {
        // lock the state for the entire block mining process
        let mut state = self.state.write().await;

        let partial_header = PartialHeader {
            gas_price: self.env.read().block.gas_price,
            number: self.env.read().block.block_number.0,
            timestamp: self.env.read().block.block_timestamp.0,
            parent_hash: self.blockchain.storage.read().latest_hash,
            sequencer_address: (*self.env.read().block.sequencer_address.0.key()).into(),
        };

        let (valid_txs, outputs): (Vec<_>, Vec<_>) = execution_outcome
            .transactions
            .iter()
            .filter_map(|tx| match tx {
                MaybeInvalidExecutedTransaction::Valid(tx) => Some((tx.clone(), tx.output.clone())),
                _ => None,
            })
            .unzip();

        let block = Block::new(partial_header, valid_txs, outputs);

        let block_number = block.header.number;
        let tx_count = block.transactions.len();
        let block_hash = block.header.hash();

        execution_outcome.transactions.iter().for_each(|tx| {
            let (hash, tx) = match tx {
                MaybeInvalidExecutedTransaction::Valid(tx) => (
                    tx.inner.hash(),
                    KnownTransaction::Included(IncludedTransaction {
                        block_number,
                        block_hash,
                        transaction: tx.clone(),
                        finality_status: TransactionFinalityStatus::AcceptedOnL2,
                    }),
                ),

                MaybeInvalidExecutedTransaction::Invalid(tx) => {
                    (tx.inner.hash(), KnownTransaction::Rejected(tx.clone()))
                }
            };

            self.blockchain.storage.write().transactions.insert(hash, tx);
        });

        // store block and the state diff
        let state_diff = convert_state_diff_to_rpc_state_diff(execution_outcome.state_diff.clone());
        self.blockchain.append_block(block_hash, block.clone(), state_diff);
        // apply the pending state to the current state
        execution_outcome.apply_to(&mut *state);
        // store the current state
        self.states.write().await.insert(block_hash, state.as_ref_db());

        info!(target: "backend", "⛏️ Block {block_number} mined with {tx_count} transactions");

        MinedBlockOutcome { block_number, transactions: execution_outcome.transactions }
    }

    pub fn update_block_context(&self) {
        let mut context_gen = self.block_context_generator.write();
        let block_context = &mut self.env.write().block;
        let current_timestamp_secs = get_current_timestamp().as_secs() as i64;

        let timestamp = if context_gen.next_block_start_time == 0 {
            (current_timestamp_secs + context_gen.block_timestamp_offset) as u64
        } else {
            let timestamp = context_gen.next_block_start_time;
            context_gen.block_timestamp_offset = timestamp as i64 - current_timestamp_secs;
            context_gen.next_block_start_time = 0;
            timestamp
        };

        block_context.block_number = block_context.block_number.next();
        block_context.block_timestamp = BlockTimestamp(timestamp);
    }

    pub fn call(
        &self,
        call: ExternalFunctionCall,
        state: impl StateReader,
    ) -> Result<CallInfo, EntryPointExecutionError> {
        let mut state = CachedState::new(state);
        let mut state = CachedState::new(MutRefState::new(&mut state));

        let call = CallEntryPoint {
            calldata: call.calldata,
            storage_address: call.contract_address,
            entry_point_selector: call.entry_point_selector,
            initial_gas: 1000000000,
            ..Default::default()
        };

        let res = call.execute(
            &mut state,
            &mut ExecutionResources::default(),
            &mut EntryPointExecutionContext::new(
                self.env.read().block.clone(),
                AccountTransactionContext::default(),
                1000000000,
            ),
        );

        if let Err(err) = &res {
            warn!(target: "backend", "Call error: {err:?}");
        }

        res
    }

    pub async fn create_empty_block(&self) -> Block {
        let parent_hash = self.blockchain.storage.read().latest_hash;
        let block_context = &self.env.read().block;

        let partial_header = PartialHeader {
            parent_hash,
            gas_price: block_context.gas_price,
            number: block_context.block_number.0,
            timestamp: block_context.block_timestamp.0,
            sequencer_address: (*block_context.sequencer_address.0.key()).into(),
        };

        Block::new(partial_header, vec![], vec![])
    }
}
