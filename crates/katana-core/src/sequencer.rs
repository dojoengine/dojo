use anyhow::Result;
use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use starknet::core::types::{BlockId, BlockTag, FeeEstimate, StateUpdate, TransactionStatus};
use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::core::{calculate_contract_address, ChainId, ClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeployAccountTransaction, Fee,
    Transaction as StarknetApiTransaction, TransactionHash, TransactionSignature,
};

use crate::sequencer_error::SequencerError;
use crate::starknet::block::StarknetBlock;
use crate::starknet::event::EmittedEvent;
use crate::starknet::transaction::ExternalFunctionCall;
use crate::starknet::{StarknetConfig, StarknetWrapper};
use crate::util::starkfelt_to_u128;

type SequencerResult<T> = Result<T, SequencerError>;

pub struct KatanaSequencer {
    pub starknet: StarknetWrapper,
}

impl KatanaSequencer {
    pub fn new(config: StarknetConfig) -> Self {
        Self { starknet: StarknetWrapper::new(config) }
    }

    // The starting point of the sequencer
    // Once we add support periodic block generation, the logic should be here.
    pub fn start(&mut self) {
        self.starknet.generate_genesis_block();
        self.starknet.generate_pending_block();
    }

    pub fn drip_and_deploy_account(
        &mut self,
        class_hash: ClassHash,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
        balance: u64,
    ) -> SequencerResult<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .map_err(SequencerError::StarknetApi)?;

        let deployed_account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()])
                .map_err(SequencerError::StarknetApi)?;

        self.starknet.pending_state.set_storage_at(
            self.starknet.block_context.fee_token_address,
            deployed_account_balance_key,
            stark_felt!(balance),
        );

        self.deploy_account(class_hash, contract_address_salt, constructor_calldata, signature)
    }
}

impl Sequencer for KatanaSequencer {
    fn deploy_account(
        &mut self,
        class_hash: ClassHash,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
    ) -> SequencerResult<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .map_err(SequencerError::StarknetApi)?;

        let account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()])
                .map_err(SequencerError::StarknetApi)?;

        let max_fee = {
            self.starknet
                .state
                .get_storage_at(self.starknet.block_context.fee_token_address, account_balance_key)
                .map_err(SequencerError::State)?
        };
        // TODO: Compute txn hash
        let tx_hash = TransactionHash::default();
        let tx = AccountTransaction::DeployAccount(DeployAccountTransaction {
            class_hash,
            contract_address,
            contract_address_salt,
            constructor_calldata,
            version: Default::default(),
            nonce: Nonce(stark_felt!(0_u8)),
            signature,
            transaction_hash: tx_hash,
            max_fee: Fee(starkfelt_to_u128(max_fee).map_err(|e| {
                SequencerError::ConversionError {
                    message: e.to_string(),
                    to: "u128".to_string(),
                    from: "StarkFelt".to_string(),
                }
            })?),
        });

        tx.execute(&mut self.starknet.pending_state, &self.starknet.block_context)
            .map_err(SequencerError::TransactionExecution)?;

        Ok((tx_hash, contract_address))
    }

    fn add_account_transaction(&mut self, transaction: AccountTransaction) {
        self.starknet.handle_transaction(Transaction::AccountTransaction(transaction));
    }

    fn estimate_fee(
        &self,
        account_transaction: AccountTransaction,
        block_id: BlockId,
    ) -> SequencerResult<FeeEstimate> {
        let state = self
            .starknet
            .state_from_block_id(block_id)
            .ok_or(SequencerError::StateNotFound(block_id))?;

        let exec_info = self
            .starknet
            .simulate_transaction(account_transaction, Some(state))
            .map_err(SequencerError::TransactionExecution)?;

        let (l1_gas_usage, vm_resources) = extract_l1_gas_and_vm_usage(&exec_info.actual_resources);
        let l1_gas_by_vm_usage =
            calculate_l1_gas_by_vm_usage(&self.starknet.block_context, &vm_resources)
                .map_err(SequencerError::TransactionExecution)?;

        let total_l1_gas_usage = l1_gas_usage as f64 + l1_gas_by_vm_usage;

        Ok(FeeEstimate {
            overall_fee: total_l1_gas_usage.ceil() as u64
                * self.starknet.block_context.gas_price as u64,
            gas_consumed: total_l1_gas_usage.ceil() as u64,
            gas_price: self.starknet.block_context.gas_price as u64,
        })
    }

    fn block_hash_and_number(&self) -> Option<(BlockHash, BlockNumber)> {
        let block = self.starknet.blocks.latest()?;
        Some((block.block_hash(), block.block_number()))
    }

    fn class_hash_at(
        &mut self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<ClassHash> {
        let mut state = self
            .starknet
            .state_from_block_id(block_id)
            .ok_or(SequencerError::StateNotFound(block_id))?;

        state.get_class_hash_at(contract_address).map_err(SequencerError::State)
    }

    fn storage_at(
        &mut self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockId,
    ) -> SequencerResult<StarkFelt> {
        let mut state = self
            .starknet
            .state_from_block_id(block_id)
            .ok_or(SequencerError::StateNotFound(block_id))?;

        state.get_storage_at(contract_address, storage_key).map_err(SequencerError::State)
    }

    fn chain_id(&self) -> ChainId {
        self.starknet.block_context.chain_id.clone()
    }

    fn block_number(&self) -> BlockNumber {
        self.starknet.block_context.block_number
    }

    fn block(&self, block_id: BlockId) -> Option<StarknetBlock> {
        match block_id {
            BlockId::Tag(BlockTag::Pending) => self.starknet.blocks.pending_block.clone(),

            id => self
                .starknet
                .block_number_from_block_id(id)
                .and_then(|n| self.starknet.blocks.by_number(n)),
        }
    }

    fn nonce_at(
        &mut self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<Nonce> {
        let mut state = self
            .starknet
            .state_from_block_id(block_id)
            .ok_or(SequencerError::StateNotFound(block_id))?;

        state.get_nonce_at(contract_address).map_err(SequencerError::State)
    }

    fn call(
        &self,
        block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> SequencerResult<Vec<StarkFelt>> {
        let state = self
            .starknet
            .state_from_block_id(block_id)
            .ok_or(SequencerError::StateNotFound(block_id))?;

        self.starknet
            .call(function_call, Some(state))
            .map_err(SequencerError::EntryPointExecution)
            .map(|execution_info| execution_info.execution.retdata.0)
    }

    fn transaction_status(&self, hash: &TransactionHash) -> Option<TransactionStatus> {
        self.starknet.transactions.by_hash(hash).map(|tx| tx.status)
    }

    fn transaction_receipt(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::TransactionReceipt> {
        self.starknet.transactions.by_hash(hash).map(|tx| tx.receipt())
    }

    fn transaction(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::Transaction> {
        self.starknet.transactions.by_hash(hash).map(|tx| tx.inner.clone())
    }

    fn events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
        address: Option<StarkFelt>,
        keys: Option<Vec<Vec<StarkFelt>>>,
        _continuation_token: Option<String>,
        _chunk_size: u64,
    ) -> SequencerResult<Vec<EmittedEvent>> {
        let from_block = self
            .starknet
            .block_number_from_block_id(from_block)
            .ok_or(SequencerError::BlockNotFound(from_block))?;

        let to_block = self
            .starknet
            .block_number_from_block_id(to_block)
            .ok_or(SequencerError::BlockNotFound(to_block))?;

        let mut events = Vec::new();
        for i in from_block.0..to_block.0 {
            let block = self
                .starknet
                .blocks
                .by_number(BlockNumber(i))
                .ok_or(SequencerError::BlockNotFound(BlockId::Number(i)))?;

            for tx in block.transactions() {
                match tx {
                    StarknetApiTransaction::Invoke(_) | StarknetApiTransaction::L1Handler(_) => {}
                    _ => continue,
                }

                let sn_tx = self
                    .starknet
                    .transactions
                    .transactions
                    .get(&tx.transaction_hash())
                    .ok_or(SequencerError::TxnNotFound(tx.transaction_hash()))?;

                events.extend(
                    sn_tx
                        .emitted_events()
                        .iter()
                        .filter(|event| {
                            // Check the address condition
                            let address_condition = match &address {
                                Some(a) => a != event.from_address.0.key(),
                                None => true,
                            };

                            // If the address condition is false, no need to check the keys
                            if !address_condition {
                                return false;
                            }

                            // Check the keys condition
                            match &keys {
                                Some(keys) => {
                                    // "Per key (by position), designate the possible values to be
                                    // matched for events to be
                                    // returned. Empty array designates 'any' value"
                                    let keys_to_check =
                                        std::cmp::min(keys.len(), event.content.keys.len());

                                    event
                                        .content
                                        .keys
                                        .iter()
                                        .zip(keys.iter())
                                        .take(keys_to_check)
                                        .all(|(key, filter)| filter.contains(&key.0))
                                }
                                None => true,
                            }
                        })
                        .map(|event| EmittedEvent {
                            inner: event.clone(),
                            block_hash: block.block_hash(),
                            block_number: block.block_number(),
                            transaction_hash: tx.transaction_hash(),
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }

        Ok(events)
    }

    fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate> {
        let block_number = self
            .starknet
            .block_number_from_block_id(block_id)
            .ok_or(SequencerError::BlockNotFound(block_id))?;

        self.starknet
            .blocks
            .get_state_update(block_number)
            .ok_or(SequencerError::StateUpdateNotFound(block_id))
    }

    fn generate_new_block(&mut self) {
        self.starknet.generate_latest_block();
        self.starknet.generate_pending_block();
    }
}

pub trait Sequencer {
    fn chain_id(&self) -> ChainId;

    fn generate_new_block(&mut self);

    fn transaction_receipt(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::TransactionReceipt>;

    fn transaction_status(&self, hash: &TransactionHash) -> Option<TransactionStatus>;

    fn nonce_at(
        &mut self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<Nonce>;

    fn block_number(&self) -> BlockNumber;

    fn block(&self, block_id: BlockId) -> Option<StarknetBlock>;

    fn transaction(&self, hash: &TransactionHash)
    -> Option<starknet_api::transaction::Transaction>;

    fn class_hash_at(
        &mut self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<ClassHash>;

    fn block_hash_and_number(&self) -> Option<(BlockHash, BlockNumber)>;

    fn call(
        &self,
        block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> SequencerResult<Vec<StarkFelt>>;

    fn storage_at(
        &mut self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockId,
    ) -> SequencerResult<StarkFelt>;

    fn deploy_account(
        &mut self,
        class_hash: ClassHash,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
    ) -> SequencerResult<(TransactionHash, ContractAddress)>;

    fn add_account_transaction(&mut self, transaction: AccountTransaction);

    fn estimate_fee(
        &self,
        account_transaction: AccountTransaction,
        block_id: BlockId,
    ) -> SequencerResult<FeeEstimate>;

    fn events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
        address: Option<StarkFelt>,
        keys: Option<Vec<Vec<StarkFelt>>>,
        _continuation_token: Option<String>,
        _chunk_size: u64,
    ) -> SequencerResult<Vec<EmittedEvent>>;

    fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate>;
}
