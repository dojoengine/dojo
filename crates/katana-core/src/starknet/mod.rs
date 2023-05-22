use std::path::PathBuf;

use anyhow::Result;
use blockifier::block_context::BlockContext;
use blockifier::execution::entry_point::{CallEntryPoint, CallInfo, ExecutionContext};
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::state::cached_state::{CachedState, CommitmentStateDiff, MutRefState};
use blockifier::state::state_api::State;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{AccountTransactionContext, TransactionExecutionInfo};
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::{DeclareTransaction, ExecutableTransaction};
use starknet::core::types::{BlockId, BlockTag, FieldElement, StateUpdate, TransactionStatus};
use starknet_api::block::{BlockHash, BlockNumber, BlockTimestamp, GasPrice};
use starknet_api::core::{ClassHash, ContractAddress, GlobalRoot, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::transaction::{DeclareTransactionV0V1, DeployTransaction, TransactionHash};
use starknet_api::{patricia_key, stark_felt};
use tracing::info;

pub mod block;
pub mod event;
pub mod transaction;

use block::{StarknetBlock, StarknetBlocks};
use transaction::{StarknetTransaction, StarknetTransactions};

use self::transaction::ExternalFunctionCall;
use crate::accounts::PredeployedAccounts;
use crate::block_context::block_context_from_config;
use crate::constants::{
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, ERC20_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS, UDC_ADDRESS,
    UDC_CLASS_HASH,
};
use crate::state::DictStateReader;
use crate::util::{
    convert_blockifier_tx_to_starknet_api_tx, convert_state_diff_to_rpc_state_diff,
    get_current_timestamp,
};

#[derive(Debug)]
pub struct StarknetConfig {
    pub seed: [u8; 32],
    pub gas_price: u128,
    pub chain_id: String,
    pub total_accounts: u8,
    pub blocks_on_demand: bool,
    pub allow_zero_max_fee: bool,
    pub account_path: Option<PathBuf>,
}

pub struct StarknetWrapper {
    pub config: StarknetConfig,
    pub blocks: StarknetBlocks,
    pub block_context: BlockContext,
    pub transactions: StarknetTransactions,
    pub state: DictStateReader,
    pub predeployed_accounts: PredeployedAccounts,
    pub pending_state: CachedState<DictStateReader>,
}

impl StarknetWrapper {
    pub fn new(config: StarknetConfig) -> Self {
        let blocks = StarknetBlocks::default();
        let block_context = block_context_from_config(&config);
        let transactions = StarknetTransactions::default();
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
            pending_state,
            predeployed_accounts,
        }
    }

    pub fn state_from_block_id(&self, block_id: BlockId) -> Option<DictStateReader> {
        match block_id {
            BlockId::Tag(BlockTag::Latest) => Some(self.latest_state()),
            BlockId::Tag(BlockTag::Pending) => Some(self.pending_state()),

            id => self.block_number_from_block_id(id).and_then(|n| self.state(n)),
        }
    }

    pub fn block_number_from_block_id(&self, block_id: BlockId) -> Option<BlockNumber> {
        match block_id {
            BlockId::Number(number) => Some(BlockNumber(number)),

            BlockId::Hash(hash) => {
                self.blocks.hash_to_num.get(&BlockHash(StarkFelt::from(hash))).cloned()
            }

            BlockId::Tag(BlockTag::Pending) => None,
            BlockId::Tag(BlockTag::Latest) => self.blocks.current_block_number(),
        }
    }

    // Simulate a transaction without modifying the state
    pub fn simulate_transaction(
        &self,
        transaction: AccountTransaction,
        state: Option<DictStateReader>,
    ) -> Result<TransactionExecutionInfo, TransactionExecutionError> {
        let mut state = CachedState::new(state.unwrap_or(self.pending_state()));
        transaction.execute(&mut state, &self.block_context)
    }

    // execute the tx
    pub fn handle_transaction(&mut self, transaction: Transaction) {
        let api_tx = convert_blockifier_tx_to_starknet_api_tx(&transaction);

        info!("Transaction received | Transaction hash: {}", api_tx.transaction_hash());

        let res = match transaction {
            Transaction::AccountTransaction(tx) => {
                self.check_tx_fee(&tx);
                tx.execute(&mut self.pending_state, &self.block_context)
            }
            Transaction::L1HandlerTransaction(tx) => {
                tx.execute(&mut self.pending_state, &self.block_context)
            }
        };

        match res {
            Ok(exec_info) => {
                let starknet_tx = StarknetTransaction::new(
                    api_tx.clone(),
                    TransactionStatus::Pending,
                    Some(exec_info),
                    None,
                );

                //  append successful tx to pending block
                self.blocks
                    .pending_block
                    .as_mut()
                    .expect("no pending block")
                    .insert_transaction(api_tx);

                self.store_transaction(starknet_tx);

                if !self.config.blocks_on_demand {
                    self.generate_latest_block();
                    self.generate_pending_block();
                }
            }

            Err(exec_err) => {
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

        new_block.inner.header.block_number = self.block_context.block_number;
        let block_hash = new_block.compute_block_hash();
        new_block.inner.header.block_hash = block_hash;

        for pending_tx in new_block.transactions() {
            let tx_hash = pending_tx.transaction_hash();

            // Update the tx block hash and number in the tx store //

            if let Some(tx) = self.transactions.transactions.get_mut(&tx_hash) {
                tx.block_hash = Some(block_hash);
                tx.status = TransactionStatus::AcceptedOnL2;
                tx.block_number = Some(new_block.block_number());
            }
        }

        info!(
            "⛏️ New block generated | Block hash: {} | Block number: {}",
            new_block.block_hash(),
            new_block.block_number()
        );

        // apply state diff
        let pending_state_diff = self.pending_state.to_state_diff();

        self.blocks.num_to_state_update.insert(
            new_block.block_number(),
            StateUpdate {
                block_hash: block_hash.0.into(),
                new_root: new_block.header().state_root.0.into(),
                old_root: if new_block.block_number() == BlockNumber(0) {
                    FieldElement::ZERO
                } else {
                    self.blocks
                        .latest()
                        .map(|last_block| last_block.header().state_root.0.into())
                        .unwrap()
                },
                state_diff: convert_state_diff_to_rpc_state_diff(pending_state_diff.clone()),
            },
        );

        // reset the pending block
        self.blocks.pending_block = None;

        // TODO: Compute state root
        self.blocks.insert(new_block);

        self.apply_state_diff_to_state(pending_state_diff);

        self.update_block_context();
    }

    pub fn generate_pending_block(&mut self) {
        self.blocks.pending_block = Some(self.create_empty_block());
        // Update the pending state to the latest committed state
        self.pending_state = CachedState::new(self.state.clone());
    }

    pub fn call(
        &self,
        call: ExternalFunctionCall,
        state: Option<DictStateReader>,
    ) -> Result<CallInfo, EntryPointExecutionError> {
        let mut state = CachedState::new(state.unwrap_or(self.pending_state()));
        let mut state = CachedState::new(MutRefState::new(&mut state));

        let call = CallEntryPoint {
            calldata: call.calldata,
            storage_address: call.contract_address,
            entry_point_selector: call.entry_point_selector,
            ..Default::default()
        };

        call.execute(
            &mut state,
            &mut ExecutionContext::new(
                self.block_context.clone(),
                AccountTransactionContext::default(),
            ),
        )
    }

    pub fn state(&self, block_number: BlockNumber) -> Option<DictStateReader> {
        self.blocks.get_state(&block_number).cloned()
    }

    pub fn pending_state(&self) -> DictStateReader {
        let mut state = self.pending_state.state.clone();
        apply_state_diff(&mut state, self.pending_state.to_state_diff());
        state
    }

    pub fn latest_state(&self) -> DictStateReader {
        self.state.clone()
    }

    fn check_tx_fee(&self, transaction: &AccountTransaction) {
        let max_fee = match transaction {
            AccountTransaction::Invoke(tx) => tx.max_fee(),
            AccountTransaction::DeployAccount(tx) => tx.max_fee,
            AccountTransaction::Declare(DeclareTransaction { tx, .. }) => match tx {
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
        self.generate_pending_block();

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

    fn create_empty_block(&self) -> StarknetBlock {
        StarknetBlock::new(
            BlockHash::default(),
            BlockHash::default(),
            BlockNumber::default(),
            GasPrice(self.block_context.gas_price),
            GlobalRoot::default(),
            self.block_context.sequencer_address,
            BlockTimestamp(get_current_timestamp().as_secs()),
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
        self.block_context.block_timestamp = BlockTimestamp(get_current_timestamp().as_secs());
    }

    // apply the pending state diff to the state
    fn apply_state_diff_to_state(&mut self, state_diff: CommitmentStateDiff) {
        let state = &mut self.state;
        apply_state_diff(state, state_diff);

        // Store the block state
        self.blocks.store_state(self.block_context.block_number, state.clone());
    }
}

fn apply_state_diff(state: &mut DictStateReader, state_diff: CommitmentStateDiff) {
    // update contract storages
    state_diff.storage_updates.into_iter().for_each(|(contract_address, storages)| {
        storages.into_iter().for_each(|(key, value)| {
            state.storage_view.insert((contract_address, key), value);
        })
    });

    // update declared contracts
    state_diff.class_hash_to_compiled_class_hash.into_iter().for_each(
        |(class_hash, compiled_class_hash)| {
            state.class_hash_to_compiled_class_hash.insert(class_hash, compiled_class_hash);
        },
    );

    // update deployed contracts
    state_diff.address_to_class_hash.into_iter().for_each(|(contract_address, class_hash)| {
        state.address_to_class_hash.insert(contract_address, class_hash);
    });

    // update accounts nonce
    state_diff.address_to_nonce.into_iter().for_each(|(contract_address, nonce)| {
        state.address_to_nonce.insert(contract_address, nonce);
    });
}
