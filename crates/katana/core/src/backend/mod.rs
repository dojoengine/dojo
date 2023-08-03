use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use blockifier::block_context::BlockContext;
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
use blockifier::transaction::transaction_execution::Transaction;
use flate2::write::GzEncoder;
use flate2::Compression;
use parking_lot::RwLock;
use starknet::core::types::{BlockId, BlockTag, FeeEstimate, FieldElement};
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

use crate::accounts::PredeployedAccounts;
use crate::backend::state::{MemDb, StateExt};
use crate::block_context::BlockContextGenerator;
use crate::constants::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use crate::db::serde::state::SerializableState;
use crate::db::Db;
use crate::sequencer_error::SequencerError;
use crate::utils::transaction::convert_blockifier_to_api_tx;
use crate::utils::{convert_state_diff_to_rpc_state_diff, get_current_timestamp};

use self::config::StarknetConfig;
use self::executor::{execute_transaction, PendingBlockExecutor};
use self::storage::block::{Block, PartialHeader};
use self::storage::transaction::{IncludedTransaction, KnownTransaction, TransactionStatus};
use self::storage::{BlockchainStorage, InMemoryBlockStates};

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
    pub block_context: Arc<RwLock<BlockContext>>,
    pub block_context_generator: RwLock<BlockContextGenerator>,
    pub state: AsyncRwLock<MemDb>,
    pub predeployed_accounts: PredeployedAccounts,
}

impl Backend {
    pub fn new(config: StarknetConfig) -> Self {
        let block_context = Arc::new(RwLock::new(config.block_context()));
        let block_context_generator = config.block_context_generator();

        let mut state = MemDb::default();

        let storage = BlockchainStorage::new(&block_context.read());
        let states = InMemoryBlockStates::default();

        if let Some(ref init_state) = config.init_state {
            state.load_state(init_state.clone()).expect("failed to load initial state");
            info!("Successfully loaded initial state");
        }

        let predeployed_accounts = PredeployedAccounts::initialize(
            config.total_accounts,
            config.seed,
            *DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
            config.account_path.clone(),
        )
        .expect("should be able to generate accounts");
        predeployed_accounts.deploy_accounts(&mut state);

        Self {
            block_context,
            state: AsyncRwLock::new(state),
            config: RwLock::new(config),
            states: AsyncRwLock::new(states),
            storage: Arc::new(AsyncRwLock::new(storage)),
            block_context_generator: RwLock::new(block_context_generator),
            pending_block: AsyncRwLock::new(None),
            predeployed_accounts,
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
            Transaction::AccountTransaction(transaction),
            &mut state,
            &self.block_context.read(),
        )?;

        if exec_info.revert_error.is_some() {
            // TEMP: change this once `Reverted` transaction error is no longer `String`.
            return Err(TransactionExecutionError::ExecutionError(
                EntryPointExecutionError::ExecutionFailed { error_data: vec![] },
            ));
        }

        let (l1_gas_usage, vm_resources) = extract_l1_gas_and_vm_usage(&exec_info.actual_resources);
        let l1_gas_by_vm_usage =
            calculate_l1_gas_by_vm_usage(&self.block_context.read(), &vm_resources)?;
        let total_l1_gas_usage = l1_gas_usage as f64 + l1_gas_by_vm_usage;

        let gas_price = self.block_context.read().gas_price as u64;

        Ok(FeeEstimate {
            gas_consumed: total_l1_gas_usage.ceil() as u64,
            gas_price,
            overall_fee: total_l1_gas_usage.ceil() as u64 * gas_price,
        })
    }

    // execute the tx
    pub async fn handle_transaction(&self, transaction: Transaction) {
        let api_tx = convert_blockifier_to_api_tx(&transaction);

        if let Transaction::AccountTransaction(tx) = &transaction {
            self.check_tx_fee(tx);
        }

        let is_valid = if let Some(pending_block) = self.pending_block.write().await.as_mut() {
            info!("Transaction received | Hash: {}", api_tx.transaction_hash());
            pending_block.add_transaction(transaction).await
        } else {
            return error!("Unable to process transaction: no pending block");
        };

        if is_valid && self.config.read().auto_mine {
            self.generate_latest_block().await;
            self.generate_pending_block().await;
        }
    }

    // Creates a new block that contains all the pending txs
    // Will update the txs status to accepted
    // Append the block to the chain
    // Update the block context
    pub async fn generate_latest_block(&self) {
        let block = match self.pending_block.read().await.as_ref().map(|p| {
            let partial_block = p.as_block();
            Block::new(partial_block.header, partial_block.transactions, partial_block.outputs)
        }) {
            Some(block) => block,
            None => self.create_empty_block().await,
        };

        let pending_txs = block.transactions.clone();

        let block_hash = block.header.hash();
        let block_number = block.header.number;

        // Store state diffs
        if let Some(pending_block) = self.pending_block.read().await.as_ref() {
            let state_diff =
                convert_state_diff_to_rpc_state_diff(pending_block.state.to_state_diff());
            self.storage.write().await.append_block(block_hash, block, state_diff);
        }

        info!("⛏️ New block generated | Hash: {block_hash:#x} | Number: {block_number}");

        // Stores the pending transaction in the storage

        {
            for pending_tx in pending_txs.into_iter() {
                let hash: FieldElement = pending_tx.transaction.transaction_hash().0.into();
                self.storage.write().await.transactions.insert(
                    hash,
                    KnownTransaction::Included(IncludedTransaction {
                        block_number,
                        block_hash,
                        transaction: pending_tx.clone(),
                        status: TransactionStatus::AcceptedOnL2,
                    }),
                );
            }
        }

        self.apply_pending_state().await;
    }

    // apply the pending state diff to the state
    async fn apply_pending_state(&self) {
        let Some(ref mut pending_block ) = *self.pending_block.write().await else {
            panic!("failed to apply pending state: no pending block")
        };

        // Apply the pending state to the current state
        self.state.write().await.apply_state(&mut pending_block.state);

        // Store the current state snapshot
        let state = self.state.read().await.clone();
        let hash = self.storage.read().await.latest_hash;
        self.states.write().await.insert(hash, state);
    }

    pub async fn generate_pending_block(&self) {
        let latest_hash = self.storage.read().await.latest_hash;
        let latest_state = self.state.read().await.clone();

        self.update_block_context();
        let _ = self.pending_block.write().await.insert(PendingBlockExecutor::new(
            latest_hash,
            latest_state,
            self.block_context.clone(),
            self.storage.clone(),
        ));
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
                self.block_context.read().clone(),
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

    fn check_tx_fee(&self, transaction: &AccountTransaction) {
        let max_fee = match transaction {
            AccountTransaction::Invoke(tx) => tx.max_fee(),
            AccountTransaction::DeployAccount(tx) => tx.max_fee,
            AccountTransaction::Declare(tx) => match tx.tx() {
                starknet_api::transaction::DeclareTransaction::V0(tx) => tx.max_fee,
                starknet_api::transaction::DeclareTransaction::V1(tx) => tx.max_fee,
                starknet_api::transaction::DeclareTransaction::V2(tx) => tx.max_fee,
            },
        };

        if !self.config.read().allow_zero_max_fee && max_fee.0 == 0 {
            panic!("max fee == 0 is not supported")
        }
    }

    pub async fn create_empty_block(&self) -> Block {
        let parent_hash = self.storage.read().await.latest_hash;

        let partial_header = PartialHeader {
            parent_hash,
            gas_price: self.block_context.read().gas_price,
            number: self.block_context.read().block_number.0,
            timestamp: self.block_context.read().block_timestamp.0,
            sequencer_address: (*self.block_context.read().sequencer_address.0.key()).into(),
        };

        Block::new(partial_header, vec![], vec![])
    }

    fn update_block_context(&self) {
        let mut block_context_gen = self.block_context_generator.write();
        let mut block_context = self.block_context.write();
        block_context.block_number = block_context.block_number.next();

        let current_timestamp_secs = get_current_timestamp().as_secs() as i64;

        if block_context_gen.next_block_start_time == 0 {
            let block_timestamp = current_timestamp_secs + block_context_gen.block_timestamp_offset;
            block_context.block_timestamp = BlockTimestamp(block_timestamp as u64);
        } else {
            let block_timestamp = block_context_gen.next_block_start_time;
            block_context_gen.block_timestamp_offset =
                block_timestamp as i64 - current_timestamp_secs;
            block_context.block_timestamp = BlockTimestamp(block_timestamp);
            block_context_gen.next_block_start_time = 0;
        }
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
