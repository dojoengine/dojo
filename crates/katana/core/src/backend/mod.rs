use anyhow::Result;
use blockifier::block_context::BlockContext;
use blockifier::execution::entry_point::{
    CallEntryPoint, CallInfo, EntryPointExecutionContext, ExecutionResources,
};
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::cached_state::{CachedState, MutRefState};
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{
    AccountTransactionContext, ResourcesMapping, TransactionExecutionInfo,
};
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use convert_case::{Case, Casing};
use parking_lot::RwLock;
use starknet::core::types::{FeeEstimate, FieldElement, StateUpdate, TransactionStatus};
use starknet_api::block::{BlockHash, BlockNumber, BlockTimestamp, GasPrice};
use starknet_api::core::{ClassHash, ContractAddress, GlobalRoot, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::transaction::{DeclareTransactionV0V1, DeployTransaction, TransactionHash};
use starknet_api::{patricia_key, stark_felt};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{info, trace, warn};

pub mod block;
pub mod config;
pub mod contract;
pub mod pending;
pub mod state;
pub mod storage;
pub mod transaction;

use block::{StarknetBlock, StarknetBlocks};
use config::StarknetConfig;
use transaction::{ExternalFunctionCall, StarknetTransaction, StarknetTransactions};

use crate::accounts::PredeployedAccounts;
use crate::backend::state::{MemDb, StateExt};
use crate::block_context::BlockContextGenerator;
use crate::constants::{
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, ERC20_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS, UDC_ADDRESS,
    UDC_CLASS_HASH,
};
use crate::sequencer_error::SequencerError;
use crate::util::{
    convert_blockifier_tx_to_starknet_api_tx, convert_state_diff_to_rpc_state_diff,
    get_current_timestamp,
};

pub struct Backend {
    pub config: RwLock<StarknetConfig>,
    pub blocks: AsyncRwLock<StarknetBlocks>,
    pub block_context: RwLock<BlockContext>,
    pub block_context_generator: RwLock<BlockContextGenerator>,
    pub transactions: AsyncRwLock<StarknetTransactions>,
    pub state: AsyncRwLock<MemDb>,
    pub predeployed_accounts: PredeployedAccounts,
    pub pending_cached_state: AsyncRwLock<CachedState<MemDb>>,
}

impl Backend {
    pub fn new(config: StarknetConfig) -> Self {
        let blocks = StarknetBlocks::default();
        let transactions = StarknetTransactions::default();

        let block_context = config.block_context();
        let block_context_generator = config.block_context_generator();

        let mut state = MemDb::default();
        let pending_state = CachedState::new(state.clone());

        let predeployed_accounts = PredeployedAccounts::initialize(
            config.total_accounts,
            config.seed,
            *DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
            config.account_path.clone(),
        )
        .expect("should be able to generate accounts");
        predeployed_accounts.deploy_accounts(&mut state);

        Self {
            state: AsyncRwLock::new(state),
            config: RwLock::new(config),
            blocks: AsyncRwLock::new(blocks),
            transactions: AsyncRwLock::new(transactions),
            block_context: RwLock::new(block_context),
            block_context_generator: RwLock::new(block_context_generator),
            pending_cached_state: AsyncRwLock::new(pending_state),
            predeployed_accounts,
        }
    }

    pub async fn estimate_fee(
        &self,
        transaction: AccountTransaction,
        state: Option<MemDb>,
    ) -> Result<FeeEstimate, TransactionExecutionError> {
        let mut state = CachedState::new(state.unwrap_or(self.pending_state().await));

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
        let api_tx = convert_blockifier_tx_to_starknet_api_tx(&transaction);

        info!("Transaction received | Hash: {}", api_tx.transaction_hash());

        if let Transaction::AccountTransaction(tx) = &transaction {
            self.check_tx_fee(tx);
        }

        let res = execute_transaction(
            transaction,
            &mut (*self.pending_cached_state.write().await),
            &self.block_context.read(),
        );

        match res {
            Ok(exec_info) => {
                trace!(
                    "Transaction resource usage: {}",
                    pretty_print_resources(&exec_info.actual_resources)
                );

                let status = if exec_info.revert_error.is_some() {
                    // TODO: change the status to `Reverted` status once the variant is implemented.
                    TransactionStatus::Rejected
                } else {
                    TransactionStatus::Pending
                };

                let starknet_tx = StarknetTransaction::new(
                    api_tx.clone(),
                    status,
                    Some(exec_info),
                    // TODO: if transaction is `Reverted`, then the `revert_error` should be
                    // stored. but right now `revert_error` is not of type
                    // `TransactionExecutionError`, so we store `None` instead.
                    None,
                );

                {
                    let mut block_guard = self.blocks.write().await;
                    let pending_block =
                        block_guard.pending_block.as_mut().expect("no pending block");

                    // Append successful tx and it's output to pending block.
                    pending_block.insert_transaction(api_tx);
                    pending_block.insert_transaction_output(starknet_tx.output());
                }

                self.store_transaction(starknet_tx).await;

                if self.config.read().auto_mine {
                    self.generate_latest_block().await;
                    self.generate_pending_block().await;
                }
            }

            Err(exec_err) => {
                warn!("Transaction validation error: {exec_err:?}");

                let tx = StarknetTransaction::new(
                    api_tx,
                    TransactionStatus::Rejected,
                    None,
                    Some(exec_err),
                );

                self.store_transaction(tx).await;
            }
        }
    }

    // Creates a new block that contains all the pending txs
    // Will update the txs status to accepted
    // Append the block to the chain
    // Update the block context
    pub async fn generate_latest_block(&self) {
        let mut new_block = if let Some(ref pending) = self.blocks.write().await.pending_block {
            pending.clone()
        } else {
            self.create_empty_block()
        };

        new_block.inner.header.block_hash = new_block.compute_block_hash();

        for pending_tx in new_block.transactions() {
            let tx_hash = pending_tx.transaction_hash();

            // Update the tx block hash and number in the tx store //

            if let Some(tx) = self.transactions.write().await.transactions.get_mut(&tx_hash) {
                tx.block_hash = Some(new_block.block_hash());
                tx.status = TransactionStatus::AcceptedOnL2;
                tx.block_number = Some(new_block.block_number());
            }
        }

        info!(
            "⛏️ New block generated | Hash: {} | Number: {}",
            new_block.block_hash(),
            new_block.block_number()
        );

        let pending_state_diff = self.pending_cached_state.write().await.to_state_diff();

        let latest_block = self.blocks.read().await.latest();

        self.blocks.write().await.num_to_state_update.insert(
            new_block.block_number(),
            StateUpdate {
                block_hash: new_block.block_hash().0.into(),
                new_root: new_block.header().state_root.0.into(),
                old_root: if new_block.block_number() == BlockNumber(0) {
                    FieldElement::ZERO
                } else {
                    latest_block.map(|last_block| last_block.header().state_root.0.into()).unwrap()
                },
                state_diff: convert_state_diff_to_rpc_state_diff(pending_state_diff),
            },
        );

        self.blocks.write().await.pending_block = None;
        self.blocks.write().await.insert(new_block);
        self.update_latest_state().await;
    }

    pub async fn generate_pending_block(&self) {
        self.update_block_context();
        self.blocks.write().await.pending_block = Some(self.create_empty_block());
        *self.pending_cached_state.write().await =
            CachedState::new(self.state.read().await.clone());
    }

    pub async fn call(
        &self,
        call: ExternalFunctionCall,
        state: Option<MemDb>,
    ) -> Result<CallInfo, EntryPointExecutionError> {
        let mut state = CachedState::new(state.unwrap_or(self.pending_state().await));
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

    pub async fn state(&self, block_number: BlockNumber) -> Option<MemDb> {
        self.blocks.write().await.get_state(&block_number).cloned()
    }

    pub async fn pending_state(&self) -> MemDb {
        let mut pending_cached_state = self.pending_cached_state.write().await;
        let mut state = pending_cached_state.state.clone();
        state.apply_state(&mut *pending_cached_state);
        state
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

    /// Generate the genesis block and append it to the chain.
    /// This block should include transactions which set the initial state of the chain.
    pub async fn generate_genesis_block(&self) {
        self.blocks.write().await.pending_block = Some(self.create_empty_block());
        *self.pending_cached_state.write().await =
            CachedState::new(self.state.read().await.clone());

        let mut transactions = vec![];
        let deploy_data =
            vec![(*UDC_CLASS_HASH, *UDC_ADDRESS), (*ERC20_CONTRACT_CLASS_HASH, *FEE_TOKEN_ADDRESS)];

        for (class_hash, address) in deploy_data.into_iter() {
            let declare_tx = starknet_api::transaction::Transaction::Declare(
                starknet_api::transaction::DeclareTransaction::V1(DeclareTransactionV0V1 {
                    class_hash: ClassHash(class_hash),
                    transaction_hash: TransactionHash(stark_felt!(self
                        .transactions
                        .read()
                        .await
                        .total()
                        as u32)),
                    ..Default::default()
                }),
            );

            self.store_transaction(StarknetTransaction {
                execution_info: None,
                execution_error: None,
                inner: declare_tx.clone(),
                block_hash: Default::default(),
                block_number: Default::default(),
                status: TransactionStatus::AcceptedOnL2,
            })
            .await;

            let deploy_tx = starknet_api::transaction::Transaction::Deploy(DeployTransaction {
                class_hash: ClassHash(class_hash),
                transaction_hash: TransactionHash(stark_felt!(
                    self.transactions.read().await.total() as u32
                )),
                contract_address: ContractAddress(patricia_key!(address)),
                ..Default::default()
            });

            self.store_transaction(StarknetTransaction {
                execution_info: None,
                execution_error: None,
                inner: deploy_tx.clone(),
                block_hash: Default::default(),
                block_number: Default::default(),
                status: TransactionStatus::AcceptedOnL2,
            })
            .await;

            transactions.push(declare_tx);
            transactions.push(deploy_tx);
        }

        self.blocks.write().await.pending_block.as_mut().unwrap().inner.body.transactions =
            transactions;

        self.generate_latest_block().await;
    }

    pub fn create_empty_block(&self) -> StarknetBlock {
        let block_context = self.block_context.read();

        StarknetBlock::new(
            BlockHash::default(),
            BlockHash::default(),
            block_context.block_number,
            GasPrice(block_context.gas_price),
            GlobalRoot::default(),
            block_context.sequencer_address,
            block_context.block_timestamp,
            Vec::default(),
            Vec::default(),
            None,
        )
    }

    // store the tx doesnt matter if its successfully executed or not
    async fn store_transaction(
        &self,
        transaction: StarknetTransaction,
    ) -> Option<StarknetTransaction> {
        self.transactions
            .write()
            .await
            .transactions
            .insert(transaction.inner.transaction_hash(), transaction)
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

    // apply the pending state diff to the state
    async fn update_latest_state(&self) {
        self.state.write().await.apply_state(&mut *self.pending_cached_state.write().await);
        let state = self.state.read().await.clone();
        self.blocks.write().await.store_state(self.block_context.read().block_number, state);
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
        match self.blocks.read().await.pending_block {
            Some(ref pending_block) => !pending_block.inner.body.transactions.is_empty(),
            None => false,
        }
    }
}

fn execute_transaction<S: StateReader>(
    transaction: Transaction,
    state: &mut CachedState<S>,
    block_context: &BlockContext,
) -> Result<TransactionExecutionInfo, TransactionExecutionError> {
    let res = match transaction {
        Transaction::AccountTransaction(tx) => tx.execute(state, block_context),
        Transaction::L1HandlerTransaction(tx) => tx.execute(state, block_context),
    };

    match res {
        Ok(exec_info) => {
            if let Some(err) = &exec_info.revert_error {
                let formatted_err = format!("{:?}", err).replace("\\n", "\n");
                warn!("Transaction execution error: {formatted_err}");
            }
            Ok(exec_info)
        }
        Err(err) => {
            warn!("Transaction validation error: {err:?}");
            Err(err)
        }
    }
}

fn pretty_print_resources(resources: &ResourcesMapping) -> String {
    let mut mapped_strings: Vec<_> = resources
        .0
        .iter()
        .filter_map(|(k, v)| match k.as_str() {
            "l1_gas_usage" => Some(format!("L1 Gas: {}", v)),
            "range_check_builtin" => Some(format!("Range Checks: {}", v)),
            "ecdsa_builtin" => Some(format!("ECDSA: {}", v)),
            "n_steps" => None,
            "pedersen_builtin" => Some(format!("Pedersen: {}", v)),
            "bitwise_builtin" => Some(format!("Bitwise: {}", v)),
            "keccak_builtin" => Some(format!("Keccak: {}", v)),
            _ => Some(format!("{}: {}", k.to_case(Case::Title), v)),
        })
        .collect::<Vec<String>>();

    // Sort the strings alphabetically
    mapped_strings.sort();

    // Prepend "Steps" if it exists, so it is always first
    if let Some(steps) = resources.0.get("n_steps") {
        mapped_strings.insert(0, format!("Steps: {}", steps));
    }

    mapped_strings.join(" | ")
}
