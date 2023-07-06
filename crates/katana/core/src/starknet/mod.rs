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
use blockifier::transaction::objects::{AccountTransactionContext, TransactionExecutionInfo};
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use starknet::core::types::{FeeEstimate, FieldElement, StateUpdate, TransactionStatus};
use starknet_api::block::{BlockHash, BlockNumber, BlockTimestamp, GasPrice};
use starknet_api::core::{ClassHash, ContractAddress, GlobalRoot, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::transaction::{DeclareTransactionV0V1, DeployTransaction, TransactionHash};
use starknet_api::{patricia_key, stark_felt};
use tracing::{info, warn};

pub mod block;
pub mod config;
pub mod contract;
pub mod event;
pub mod transaction;

use block::{StarknetBlock, StarknetBlocks};
use config::StarknetConfig;
use transaction::{ExternalFunctionCall, StarknetTransaction, StarknetTransactions};

use crate::accounts::PredeployedAccounts;
use crate::block_context::BlockContextGenerator;
use crate::constants::{
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, ERC20_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS, UDC_ADDRESS,
    UDC_CLASS_HASH,
};
use crate::sequencer_error::SequencerError;
use crate::state::DictStateReader;
use crate::util::{
    convert_blockifier_tx_to_starknet_api_tx, convert_state_diff_to_rpc_state_diff,
    get_current_timestamp,
};

pub struct StarknetWrapper {
    pub config: StarknetConfig,
    pub blocks: StarknetBlocks,
    pub block_context: BlockContext,
    pub block_context_generator: BlockContextGenerator,
    pub transactions: StarknetTransactions,
    pub state: DictStateReader,
    pub predeployed_accounts: PredeployedAccounts,
    pub pending_cached_state: CachedState<DictStateReader>,
}

impl StarknetWrapper {
    pub fn new(config: StarknetConfig) -> Self {
        let blocks = StarknetBlocks::default();
        let transactions = StarknetTransactions::default();

        let block_context = config.block_context();
        let block_context_generator = config.block_context_generator();

        let mut state = DictStateReader::default();
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
            state,
            config,
            blocks,
            transactions,
            block_context,
            block_context_generator,
            pending_cached_state: pending_state,
            predeployed_accounts,
        }
    }

    pub fn estimate_fee(
        &mut self,
        transaction: AccountTransaction,
        state: Option<DictStateReader>,
    ) -> Result<FeeEstimate, TransactionExecutionError> {
        let mut state = CachedState::new(state.unwrap_or(self.pending_state()));

        let exec_info = execute_transaction(
            Transaction::AccountTransaction(transaction),
            &mut state,
            &self.block_context,
        )?;

        if exec_info.revert_error.is_some() {
            // TEMP: change this once `Reverted` transaction error is no longer `String`.
            return Err(TransactionExecutionError::ExecutionError(
                EntryPointExecutionError::ExecutionFailed { error_data: vec![] },
            ));
        }

        let (l1_gas_usage, vm_resources) = extract_l1_gas_and_vm_usage(&exec_info.actual_resources);
        let l1_gas_by_vm_usage = calculate_l1_gas_by_vm_usage(&self.block_context, &vm_resources)?;
        let total_l1_gas_usage = l1_gas_usage as f64 + l1_gas_by_vm_usage;

        Ok(FeeEstimate {
            gas_consumed: total_l1_gas_usage.ceil() as u64,
            gas_price: self.block_context.gas_price as u64,
            overall_fee: total_l1_gas_usage.ceil() as u64 * self.block_context.gas_price as u64,
        })
    }

    // execute the tx
    pub fn handle_transaction(&mut self, transaction: Transaction) {
        let api_tx = convert_blockifier_tx_to_starknet_api_tx(&transaction);

        info!("Transaction received | Hash: {}", api_tx.transaction_hash());

        if let Transaction::AccountTransaction(tx) = &transaction {
            self.check_tx_fee(tx);
        }

        let res =
            execute_transaction(transaction, &mut self.pending_cached_state, &self.block_context);

        match res {
            Ok(exec_info) => {
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

                let pending_block = self.blocks.pending_block.as_mut().expect("no pending block");

                // Append successful tx and it's output to pending block.
                pending_block.insert_transaction(api_tx);
                pending_block.insert_transaction_output(starknet_tx.output());

                self.store_transaction(starknet_tx);

                if self.config.auto_mine {
                    self.generate_latest_block();
                    self.generate_pending_block();
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

                self.store_transaction(tx);
            }
        }
    }

    // Creates a new block that contains all the pending txs
    // Will update the txs status to accepted
    // Append the block to the chain
    // Update the block context
    pub fn generate_latest_block(&mut self) {
        let mut new_block = if let Some(ref pending) = self.blocks.pending_block {
            pending.clone()
        } else {
            self.create_empty_block()
        };

        new_block.inner.header.block_hash = new_block.compute_block_hash();

        for pending_tx in new_block.transactions() {
            let tx_hash = pending_tx.transaction_hash();

            // Update the tx block hash and number in the tx store //

            if let Some(tx) = self.transactions.transactions.get_mut(&tx_hash) {
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

        let pending_state_diff = self.pending_cached_state.to_state_diff();

        self.blocks.num_to_state_update.insert(
            new_block.block_number(),
            StateUpdate {
                block_hash: new_block.block_hash().0.into(),
                new_root: new_block.header().state_root.0.into(),
                old_root: if new_block.block_number() == BlockNumber(0) {
                    FieldElement::ZERO
                } else {
                    self.blocks
                        .latest()
                        .map(|last_block| last_block.header().state_root.0.into())
                        .unwrap()
                },
                state_diff: convert_state_diff_to_rpc_state_diff(pending_state_diff),
            },
        );

        self.blocks.pending_block = None;
        self.blocks.insert(new_block);
        self.update_latest_state();
    }

    pub fn generate_pending_block(&mut self) {
        self.update_block_context();
        self.blocks.pending_block = Some(self.create_empty_block());
        self.pending_cached_state = CachedState::new(self.state.clone());
    }

    pub fn call(
        &mut self,
        call: ExternalFunctionCall,
        state: Option<DictStateReader>,
    ) -> Result<CallInfo, EntryPointExecutionError> {
        let mut state = CachedState::new(state.unwrap_or(self.pending_state()));
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
                self.block_context.clone(),
                AccountTransactionContext::default(),
                1000000000,
            ),
        );

        if let Err(err) = &res {
            warn!("Call error: {err:?}");
        }

        res
    }

    pub fn state(&self, block_number: BlockNumber) -> Option<DictStateReader> {
        self.blocks.get_state(&block_number).cloned()
    }

    pub fn pending_state(&mut self) -> DictStateReader {
        let mut state = self.pending_cached_state.state.clone();
        apply_new_state(&mut state, &mut self.pending_cached_state);
        state
    }

    pub fn latest_state(&self) -> DictStateReader {
        self.state.clone()
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

        if !self.config.allow_zero_max_fee && max_fee.0 == 0 {
            panic!("max fee == 0 is not supported")
        }
    }

    /// Generate the genesis block and append it to the chain.
    /// This block should include transactions which set the initial state of the chain.
    pub fn generate_genesis_block(&mut self) {
        self.blocks.pending_block = Some(self.create_empty_block());
        self.pending_cached_state = CachedState::new(self.state.clone());

        let mut transactions = vec![];
        let deploy_data =
            vec![(*UDC_CLASS_HASH, *UDC_ADDRESS), (*ERC20_CONTRACT_CLASS_HASH, *FEE_TOKEN_ADDRESS)];

        deploy_data.into_iter().for_each(|(class_hash, address)| {
            let declare_tx = starknet_api::transaction::Transaction::Declare(
                starknet_api::transaction::DeclareTransaction::V1(DeclareTransactionV0V1 {
                    class_hash: ClassHash(class_hash),
                    transaction_hash: TransactionHash(
                        stark_felt!(self.transactions.total() as u32),
                    ),
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
            });

            let deploy_tx = starknet_api::transaction::Transaction::Deploy(DeployTransaction {
                class_hash: ClassHash(class_hash),
                transaction_hash: TransactionHash(stark_felt!(self.transactions.total() as u32)),
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
            });

            transactions.push(declare_tx);
            transactions.push(deploy_tx);
        });

        self.blocks.pending_block.as_mut().unwrap().inner.body.transactions = transactions;

        self.generate_latest_block();
    }

    pub fn create_empty_block(&self) -> StarknetBlock {
        StarknetBlock::new(
            BlockHash::default(),
            BlockHash::default(),
            self.block_context.block_number,
            GasPrice(self.block_context.gas_price),
            GlobalRoot::default(),
            self.block_context.sequencer_address,
            self.block_context.block_timestamp,
            Vec::default(),
            Vec::default(),
            None,
        )
    }

    // store the tx doesnt matter if its successfully executed or not
    fn store_transaction(
        &mut self,
        transaction: StarknetTransaction,
    ) -> Option<StarknetTransaction> {
        self.transactions.transactions.insert(transaction.inner.transaction_hash(), transaction)
    }

    fn update_block_context(&mut self) {
        self.block_context.block_number = self.block_context.block_number.next();

        let current_timestamp_secs = get_current_timestamp().as_secs() as i64;

        if self.block_context_generator.next_block_start_time == 0 {
            let block_timestamp =
                current_timestamp_secs + self.block_context_generator.block_timestamp_offset;
            self.block_context.block_timestamp = BlockTimestamp(block_timestamp as u64);
        } else {
            let block_timestamp = self.block_context_generator.next_block_start_time;
            self.block_context_generator.block_timestamp_offset =
                block_timestamp as i64 - current_timestamp_secs;
            self.block_context.block_timestamp = BlockTimestamp(block_timestamp);
            self.block_context_generator.next_block_start_time = 0;
        }
    }

    // apply the pending state diff to the state
    fn update_latest_state(&mut self) {
        let state = &mut self.state;
        apply_new_state(state, &mut self.pending_cached_state);
        self.blocks.store_state(self.block_context.block_number, state.clone());
    }

    pub fn set_next_block_timestamp(&mut self, timestamp: u64) -> Result<(), SequencerError> {
        if has_pending_transactions(self) {
            return Err(SequencerError::PendingTransactions);
        }
        self.block_context_generator.next_block_start_time = timestamp;
        Ok(())
    }

    pub fn increase_next_block_timestamp(&mut self, timestamp: u64) -> Result<(), SequencerError> {
        if has_pending_transactions(self) {
            return Err(SequencerError::PendingTransactions);
        }
        self.block_context_generator.block_timestamp_offset += timestamp as i64;
        Ok(())
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
                warn!("Transaction execution error: {err:?}");
            }
            Ok(exec_info)
        }
        Err(err) => {
            warn!("Transaction validation error: {err:?}");
            Err(err)
        }
    }
}

fn has_pending_transactions(starknet: &StarknetWrapper) -> bool {
    match starknet.blocks.pending_block {
        Some(ref pending_block) => !pending_block.inner.body.transactions.is_empty(),
        None => false,
    }
}

fn apply_new_state(old_state: &mut DictStateReader, new_state: &mut CachedState<DictStateReader>) {
    let state_diff = new_state.to_state_diff();

    // update contract storages
    state_diff.storage_updates.into_iter().for_each(|(contract_address, storages)| {
        storages.into_iter().for_each(|(key, value)| {
            old_state.storage_view.insert((contract_address, key), value);
        })
    });

    // update declared contracts
    // apply newly declared classses
    for (class_hash, compiled_class_hash) in &state_diff.class_hash_to_compiled_class_hash {
        let contract_class =
            new_state.get_compiled_contract_class(class_hash).expect("contract class should exist");
        old_state.class_hash_to_compiled_class_hash.insert(*class_hash, *compiled_class_hash);
        old_state.class_hash_to_class.insert(*class_hash, contract_class);
    }

    // update deployed contracts
    state_diff.address_to_class_hash.into_iter().for_each(|(contract_address, class_hash)| {
        old_state.address_to_class_hash.insert(contract_address, class_hash);
    });

    // update accounts nonce
    state_diff.address_to_nonce.into_iter().for_each(|(contract_address, nonce)| {
        old_state.address_to_nonce.insert(contract_address, nonce);
    });
}
