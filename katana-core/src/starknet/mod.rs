use std::{path::PathBuf, time::SystemTime};

use anyhow::Result;
use blockifier::{
    block_context::BlockContext,
    execution::entry_point::{CallEntryPoint, CallInfo, ExecutionContext, ExecutionResources},
    state::{
        cached_state::{CachedState, MutRefState},
        state_api::State,
    },
    transaction::{
        objects::AccountTransactionContext, transaction_execution::Transaction,
        transactions::ExecutableTransaction,
    },
};
use starknet::core::types::TransactionStatus;
use starknet_api::{
    block::{BlockHash, BlockNumber, BlockTimestamp, GasPrice},
    core::GlobalRoot,
    hash::StarkFelt,
    stark_felt,
};
use tracing::info;

pub mod block;
pub mod transaction;

use crate::{
    accounts::PredeployedAccounts,
    block_context::Base,
    constants::DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
    state::DictStateReader,
    util::{
        blockifier_contract_class_from_flattened_sierra_class,
        convert_blockifier_tx_to_starknet_api_tx,
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
        let mut state = CachedState::new(DictStateReader::get_default());

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
    pub fn handle_transaction(&mut self, transaction: Transaction) {
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
                self.generate_latest_block();

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
    }

    // Creates a new block that contains all the pending txs
    // Will update the txs status to accepted
    // Append the block to the chain
    // Update the block context
    pub fn generate_latest_block(&mut self) -> StarknetBlock {
        let mut latest_block = if let Some(ref pending) = self.blocks.pending_block {
            pending.clone()
        } else {
            self.create_empty_block()
        };

        let block_hash = latest_block.compute_block_hash();
        latest_block.0.header.block_hash = block_hash;

        for pending_tx in latest_block.transactions() {
            let tx_hash = pending_tx.transaction_hash();

            if let Some(tx) = self.transactions.transactions.get_mut(&tx_hash) {
                tx.block_hash = Some(block_hash);
                tx.status = TransactionStatus::AcceptedOnL2;
                tx.block_number = Some(latest_block.block_number());
            }
        }

        info!(
            "New block generated | Block hash: {} | Block number: {}",
            latest_block.block_hash(),
            latest_block.block_number()
        );

        // reset the pending block
        self.blocks.pending_block = None;
        self.blocks.append_block(latest_block.clone());
        self.update_block_context();
        self.update_latest_state();

        latest_block
    }

    pub fn generate_pending_block(&mut self) {
        self.blocks.pending_block = Some(self.create_empty_block());
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
            &mut ExecutionResources::default(),
            &mut ExecutionContext::default(),
            &self.block_context,
            &AccountTransactionContext::default(),
        )
        .map_err(|e| e.into())
    }

    // Returns the StarknetState of the underlying Starknet instance.
    #[allow(unused)]
    fn get_state(&self) -> &DictStateReader {
        unimplemented!("StarknetWrapper::get_state")
    }

    fn create_empty_block(&self) -> StarknetBlock {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|t| BlockTimestamp(t.as_secs()))
            .expect("should get unix timestamp");

        let block_number = self.blocks.current_height;
        let parent_hash = if block_number.0 == 0 {
            BlockHash(stark_felt!(0))
        } else {
            self.blocks
                .lastest()
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
            timestamp,
            vec![],
            vec![],
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
        let next_block_number = BlockNumber(self.blocks.current_height.0 + 1);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.blocks.current_height = next_block_number;
        self.block_context.block_number = next_block_number;
        self.block_context.block_timestamp = BlockTimestamp(timestamp);
    }

    fn update_latest_state(&mut self) {
        let state_diff = self.state.to_state_diff();
        let state = &mut self.state.state;

        // update contract storages

        state_diff
            .storage_diffs
            .into_iter()
            .for_each(|(contract_address, storages)| {
                storages.into_iter().for_each(|(key, value)| {
                    state.storage_view.insert((contract_address, key), value);
                })
            });

        // update declared contracts

        state_diff.declared_classes.into_iter().for_each(
            |(class_hash, (compiled_class_hash, contract_class))| {
                let raw_contract_class = serde_json::to_string(&contract_class).unwrap();
                let contract_class =
                    blockifier_contract_class_from_flattened_sierra_class(&raw_contract_class)
                        .expect("get_blockifier_contract_class_from_flattened_sierra_class");

                state.class_hash_to_class.insert(class_hash, contract_class);

                state
                    .class_hash_to_compiled_class_hash
                    .insert(class_hash, compiled_class_hash);
            },
        );

        // update deployed contracts

        state_diff
            .deployed_contracts
            .into_iter()
            .for_each(|(contract_address, class_hash)| {
                state
                    .address_to_class_hash
                    .insert(contract_address, class_hash);
            });

        // update accounts nonce

        state_diff
            .nonces
            .into_iter()
            .for_each(|(contract_address, nonce)| {
                state.address_to_nonce.insert(contract_address, nonce);
            });
    }
}
