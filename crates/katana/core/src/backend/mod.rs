use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use blockifier::execution::entry_point::{
    CallEntryPoint, CallInfo, EntryPointExecutionContext, ExecutionResources,
};
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::cached_state::{CachedState, MutRefState};
use blockifier::state::state_api::State;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::AccountTransactionContext;
use blockifier::transaction::transaction_execution::Transaction as ExecutionTransaction;
use flate2::write::GzEncoder;
use flate2::Compression;
use parking_lot::RwLock;
use starknet::core::types::{BlockId, BlockTag, FeeEstimate};
use starknet_api::block::BlockTimestamp;
use starknet_api::core::{ContractAddress, EntryPointSelector};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;
use starknet_api::transaction::Calldata;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{error, info, warn};

pub mod config;
pub mod contract;
pub mod executor;
pub mod state;
pub mod storage;

use self::config::StarknetConfig;
use self::executor::{execute_transaction, PendingBlockExecutor};
use self::storage::block::{Block, PartialHeader};
use self::storage::transaction::{IncludedTransaction, Transaction, TransactionStatus};
use self::storage::{BlockchainStorage, InMemoryBlockStates};
use crate::accounts::{Account, DevAccountGenerator};
use crate::backend::state::{MemDb, StateExt};
use crate::constants::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use crate::db::serde::state::SerializableState;
use crate::db::Db;
use crate::env::{BlockContextGenerator, Env};
use crate::sequencer_error::SequencerError;
use crate::utils::{convert_state_diff_to_rpc_state_diff, get_current_timestamp};

pub struct ExternalFunctionCall {
    pub calldata: Calldata,
    pub contract_address: ContractAddress,
    pub entry_point_selector: EntryPointSelector,
}

pub struct Backend {
    pub config: RwLock<StarknetConfig>,
    /// stores all block related data in memory
    pub storage: Arc<AsyncRwLock<BlockchainStorage>>,
    // TEMP: pending block for transaction execution
    pub pending_block: AsyncRwLock<Option<PendingBlockExecutor>>,
    /// Historic states of previous blocks
    pub states: AsyncRwLock<InMemoryBlockStates>,
    /// The chain environment values.
    pub env: Arc<RwLock<Env>>,
    pub block_context_generator: RwLock<BlockContextGenerator>,
    pub state: AsyncRwLock<MemDb>,
    /// Prefunded dev accounts
    pub accounts: Vec<Account>,
}

impl Backend {
    pub fn new(config: StarknetConfig) -> Self {
        let block_context = config.block_context();
        let block_context_generator = config.block_context_generator();

        let mut state = MemDb::default();

        let storage = BlockchainStorage::new(&block_context);
        let states = InMemoryBlockStates::default();
        let env = Env { block: block_context };

        if let Some(ref init_state) = config.init_state {
            state.load_state(init_state.clone()).expect("failed to load initial state");
            info!("Successfully loaded initial state");
        }

        let accounts = DevAccountGenerator::new(config.total_accounts)
            .with_seed(config.seed)
            .with_balance((*DEFAULT_PREFUNDED_ACCOUNT_BALANCE).into())
            .generate();

        for acc in &accounts {
            acc.deploy_and_fund(&mut state).expect("should be able to deploy and fund dev account");
        }

        Self {
            env: Arc::new(RwLock::new(env)),
            state: AsyncRwLock::new(state),
            config: RwLock::new(config),
            states: AsyncRwLock::new(states),
            storage: Arc::new(AsyncRwLock::new(storage)),
            block_context_generator: RwLock::new(block_context_generator),
            pending_block: AsyncRwLock::new(None),
            accounts,
        }
    }

    /// Get the current state.
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
        transaction: AccountTransaction,
        state: MemDb,
    ) -> Result<FeeEstimate, TransactionExecutionError> {
        let mut state = CachedState::new(state);

        let exec_info = execute_transaction(
            ExecutionTransaction::AccountTransaction(transaction),
            &mut state,
            &self.env.read().block,
            true,
        )?;

        if exec_info.revert_error.is_some() {
            // TEMP: change this once `Reverted` transaction error is no longer `String`.
            return Err(TransactionExecutionError::ExecutionError(
                EntryPointExecutionError::ExecutionFailed { error_data: vec![] },
            ));
        }

        let (l1_gas_usage, vm_resources) = extract_l1_gas_and_vm_usage(&exec_info.actual_resources);
        let l1_gas_by_vm_usage =
            calculate_l1_gas_by_vm_usage(&self.env.read().block, &vm_resources)?;
        let total_l1_gas_usage = l1_gas_usage as f64 + l1_gas_by_vm_usage;

        let gas_price = self.env.read().block.gas_price as u64;

        Ok(FeeEstimate {
            gas_consumed: total_l1_gas_usage.ceil() as u64,
            gas_price,
            overall_fee: total_l1_gas_usage.ceil() as u64 * gas_price,
        })
    }

    // execute the tx
    pub async fn handle_transaction(&self, transaction: Transaction) {
        let is_valid = if let Some(pending_block) = self.pending_block.write().await.as_mut() {
            let charge_fee = !self.config.read().disable_fee;
            pending_block.add_transaction(transaction, charge_fee).await
        } else {
            return error!("Unable to process transaction: no pending block");
        };

        if is_valid && self.config.read().auto_mine {
            self.mine_block().await;
            self.open_pending_block().await;
        }
    }

    // Generates a new block from the pending block and stores it in the storage.
    pub async fn mine_block(&self) {
        let pending = self.pending_block.write().await.take();

        let Some(mut pending) = pending else {
            warn!("No pending block to mine");
            return;
        };

        let block = {
            let partial_block = pending.as_block();
            Block::new(partial_block.header, partial_block.transactions, partial_block.outputs)
        };

        let block_hash = block.header.hash();
        let block_number = block.header.number;
        let tx_count = block.transactions.len();

        // Stores the pending transaction in storage
        for tx in &block.transactions {
            let transaction_hash = tx.inner.hash();
            self.storage.write().await.transactions.insert(
                transaction_hash,
                IncludedTransaction {
                    block_number,
                    block_hash,
                    transaction: tx.clone(),
                    status: TransactionStatus::AcceptedOnL2,
                }
                .into(),
            );
        }

        // get state diffs
        let pending_state_diff = pending.state.to_state_diff();
        let state_diff = convert_state_diff_to_rpc_state_diff(pending_state_diff);

        // store block and the state diff
        self.storage.write().await.append_block(block_hash, block, state_diff);

        info!("⛏️ Block {block_number} mined with {tx_count} transactions");

        // apply the pending state to the current state
        self.state.write().await.apply_state(&mut pending.state);

        // store the current state
        let state = self.state.read().await.clone();
        self.states.write().await.insert(block_hash, state);
    }

    pub async fn open_pending_block(&self) {
        let latest_hash = self.storage.read().await.latest_hash;
        let latest_state = self.state.read().await.clone();

        self.update_block_context();

        let _ = self.pending_block.write().await.insert(PendingBlockExecutor::new(
            latest_hash,
            latest_state,
            self.env.clone(),
            self.storage.clone(),
        ));
    }

    fn update_block_context(&self) {
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
        state: MemDb,
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
            warn!("Call error: {err:?}");
        }

        res
    }

    pub async fn pending_state(&self) -> Option<MemDb> {
        let Some(ref mut block) = *self.pending_block.write().await else {
            return None;
        };

        let mut current_state = self.state.read().await.clone();
        current_state.apply_state(&mut block.state);
        Some(current_state)
    }

    pub async fn latest_state(&self) -> MemDb {
        self.state.read().await.clone()
    }

    pub async fn create_empty_block(&self) -> Block {
        let parent_hash = self.storage.read().await.latest_hash;
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

    pub async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), SequencerError> {
        if self.has_pending_transactions().await {
            return Err(SequencerError::PendingTransactions);
        }
        self.block_context_generator.write().next_block_start_time = timestamp;
        Ok(())
    }

    pub async fn increase_next_block_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<(), SequencerError> {
        if self.has_pending_transactions().await {
            return Err(SequencerError::PendingTransactions);
        }
        self.block_context_generator.write().block_timestamp_offset += timestamp as i64;
        Ok(())
    }

    async fn has_pending_transactions(&self) -> bool {
        if let Some(ref block) = *self.pending_block.read().await {
            !block.transactions.is_empty()
        } else {
            false
        }
    }

    pub async fn set_storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        value: StarkFelt,
    ) -> Result<(), SequencerError> {
        match self.pending_block.write().await.as_mut() {
            Some(pending_block) => {
                pending_block.state.set_storage_at(contract_address, storage_key, value);
                Ok(())
            }
            None => Err(SequencerError::StateNotFound(BlockId::Tag(BlockTag::Pending))),
        }
    }
}
