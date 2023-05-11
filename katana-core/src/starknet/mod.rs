use std::path::PathBuf;

use anyhow::Result;
use blockifier::{
    block_context::BlockContext,
    execution::entry_point::{CallEntryPoint, CallInfo, ExecutionContext},
    state::{
        cached_state::{CachedState, CommitmentStateDiff, MutRefState},
        state_api::State,
    },
    transaction::{
        objects::AccountTransactionContext, transaction_execution::Transaction,
        transactions::ExecutableTransaction,
    },
};
use starknet::{
    core::types::{FieldElement, TransactionStatus},
    providers::jsonrpc::models::StateUpdate,
};
use starknet_api::{
    block::{BlockHash, BlockNumber, BlockTimestamp, GasPrice},
    core::GlobalRoot,
    hash::StarkFelt,
    stark_felt,
};
use tracing::info;

pub mod block;
pub mod event;
pub mod transaction;

use crate::{
    accounts::PredeployedAccounts,
    block_context::Base,
    constants::DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
    state::DictStateReader,
    util::{
        convert_blockifier_tx_to_starknet_api_tx, convert_state_diff_to_rpc_state_diff,
        get_current_timestamp,
    },
};
use block::{StarknetBlock, StarknetBlocks};
use transaction::{StarknetTransaction, StarknetTransactions};

use self::transaction::ExternalFunctionCall;

pub struct StarknetConfig {
    pub total_accounts: u8,
    pub account_path: Option<PathBuf>,
}

pub struct StarknetWrapper {
    pub config: StarknetConfig,
    pub blocks: StarknetBlocks,
    pub block_context: BlockContext,
    pub transactions: StarknetTransactions,
    pub state: CachedState<DictStateReader>,
    pub predeployed_accounts: PredeployedAccounts,
}

impl StarknetWrapper {
    pub fn new(config: StarknetConfig) -> Self {
        let blocks = StarknetBlocks::default();
        let block_context = BlockContext::base();
        let transactions = StarknetTransactions::default();
        let mut state = CachedState::new(DictStateReader::default());

        let predeployed_accounts = PredeployedAccounts::generate(
            config.total_accounts,
            [0u8; 32],
            stark_felt!(DEFAULT_PREFUNDED_ACCOUNT_BALANCE),
            config
                .account_path
                .clone()
                .unwrap_or(PredeployedAccounts::default_account_class_path()),
        )
        .expect("should be able to generate accounts");
        predeployed_accounts.deploy_accounts(&mut state.state);

        Self {
            state,
            config,
            blocks,
            transactions,
            block_context,
            predeployed_accounts,
        }
    }

    // execute the tx
    pub fn handle_transaction(&mut self, transaction: Transaction) -> Result<()> {
        let api_tx = convert_blockifier_tx_to_starknet_api_tx(&transaction);

        info!(
            "Transaction received | Transaction hash: {}",
            api_tx.transaction_hash()
        );

        let res = match transaction {
            Transaction::AccountTransaction(tx) => tx.execute(&mut self.state, &self.block_context),
            Transaction::L1HandlerTransaction(tx) => {
                tx.execute(&mut self.state, &self.block_context)
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
                self.generate_latest_block()?;

                self.generate_pending_block();
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

        Ok(())
    }

    // Creates a new block that contains all the pending txs
    // Will update the txs status to accepted
    // Append the block to the chain
    // Update the block context
    pub fn generate_latest_block(&mut self) -> Result<StarknetBlock> {
        let mut new_block = if let Some(ref pending) = self.blocks.pending_block {
            pending.clone()
        } else {
            self.create_new_empty_block()
        };

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
        let state_diff = self.state.to_state_diff();

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
                state_diff: convert_state_diff_to_rpc_state_diff(state_diff.clone()),
            },
        );

        // reset the pending block
        self.blocks.pending_block = None;
        self.blocks.append_block(new_block.clone())?;
        self.update_block_context();
        // TODO: Compute state root
        self.apply_state_diff(state_diff);

        Ok(new_block)
    }

    pub fn generate_pending_block(&mut self) {
        self.blocks.pending_block = Some(self.create_new_empty_block());
    }

    // TODO: perform call based on specific block state
    pub fn call(&self, call: ExternalFunctionCall) -> Result<CallInfo> {
        let mut state = CachedState::new(self.state.state.clone());
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
        .map_err(|e| e.into())
    }

    // Returns the StarknetState of the underlying Starknet instance.
    #[allow(unused)]
    fn state(&self) -> &DictStateReader {
        unimplemented!("StarknetWrapper::state")
    }

    fn create_new_empty_block(&self) -> StarknetBlock {
        let block_number = self.block_context.block_number;

        let parent_hash = if block_number.0 == 0 {
            BlockHash(stark_felt!(0))
        } else {
            self.blocks
                .latest()
                .map(|last_block| last_block.block_hash())
                .unwrap()
        };

        StarknetBlock::new(
            BlockHash(stark_felt!(0)),
            parent_hash,
            block_number,
            GasPrice(self.block_context.gas_price),
            GlobalRoot(stark_felt!(0)),
            self.block_context.sequencer_address,
            BlockTimestamp(get_current_timestamp().as_secs()),
            vec![],
            vec![],
            None,
        )
    }

    // store the tx doesnt matter if its successfully executed or not
    fn store_transaction(
        &mut self,
        transaction: StarknetTransaction,
    ) -> Option<StarknetTransaction> {
        self.transactions
            .transactions
            .insert(transaction.inner.transaction_hash(), transaction)
    }

    fn update_block_context(&mut self) {
        self.block_context.block_number = self.block_context.block_number.next();
        self.block_context.block_timestamp = BlockTimestamp(get_current_timestamp().as_secs());
    }

    fn apply_state_diff(&mut self, state_diff: CommitmentStateDiff) {
        let state = &mut self.state.state;

        // update contract storages
        state_diff
            .storage_updates
            .into_iter()
            .for_each(|(contract_address, storages)| {
                storages.into_iter().for_each(|(key, value)| {
                    state.storage_view.insert((contract_address, key), value);
                })
            });

        // update declared contracts
        state_diff
            .class_hash_to_compiled_class_hash
            .into_iter()
            .for_each(|(class_hash, compiled_class_hash)| {
                state
                    .class_hash_to_compiled_class_hash
                    .insert(class_hash, compiled_class_hash);
            });

        // update deployed contracts
        state_diff
            .address_to_class_hash
            .into_iter()
            .for_each(|(contract_address, class_hash)| {
                state
                    .address_to_class_hash
                    .insert(contract_address, class_hash);
            });

        // update accounts nonce
        state_diff
            .address_to_nonce
            .into_iter()
            .for_each(|(contract_address, nonce)| {
                state.address_to_nonce.insert(contract_address, nonce);
            });
    }
}
