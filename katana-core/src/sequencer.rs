use anyhow::Result;
use starknet::providers::jsonrpc::models::{BlockId, BlockTag};

use crate::{
    starknet::{transaction::ExternalFunctionCall, StarknetConfig, StarknetWrapper},
    util::{field_element_to_starkfelt, starkfelt_to_u128},
};

use blockifier::{
    abi::abi_utils::get_storage_var_address,
    state::state_api::{State, StateReader},
    transaction::{
        account_transaction::AccountTransaction, transaction_execution::Transaction,
        transactions::ExecutableTransaction,
    },
};
// use starknet::providers::jsonrpc::models::BlockId;
use starknet_api::{
    block::{Block, BlockHash, BlockNumber},
    core::{calculate_contract_address, ChainId, ClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
    stark_felt,
    state::StorageKey,
    transaction::{
        Calldata, ContractAddressSalt, DeployAccountTransaction, Fee, TransactionHash,
        TransactionSignature, TransactionVersion,
    },
};

pub struct KatanaSequencer {
    pub starknet: StarknetWrapper,
}

impl KatanaSequencer {
    pub fn new(config: StarknetConfig) -> Self {
        Self {
            starknet: StarknetWrapper::new(config),
        }
    }

    // The starting point of the sequencer
    // Once we add support periodic block generation, the logic should be here.
    pub fn start(&mut self) {
        self.starknet.generate_pending_block();
    }

    pub fn drip_and_deploy_account(
        &mut self,
        class_hash: ClassHash,
        version: TransactionVersion,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
        balance: u64,
    ) -> anyhow::Result<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .unwrap();

        let deployed_account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()]).unwrap();
        self.starknet.state.set_storage_at(
            self.starknet.block_context.fee_token_address,
            deployed_account_balance_key,
            stark_felt!(balance),
        );

        self.deploy_account(
            class_hash,
            version,
            contract_address_salt,
            constructor_calldata,
            signature,
        )
    }
}

impl Sequencer for KatanaSequencer {
    fn deploy_account(
        &mut self,
        class_hash: ClassHash,
        version: TransactionVersion,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
    ) -> anyhow::Result<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .unwrap();

        let account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()]).unwrap();
        let max_fee = {
            self.starknet.state.get_storage_at(
                self.starknet.block_context.fee_token_address,
                account_balance_key,
            )?
        };
        // TODO: Compute txn hash
        let tx_hash = TransactionHash::default();
        let tx = AccountTransaction::DeployAccount(DeployAccountTransaction {
            max_fee: Fee(starkfelt_to_u128(max_fee)?),
            version,
            class_hash,
            contract_address,
            contract_address_salt,
            constructor_calldata,
            nonce: Nonce(stark_felt!(0)),
            signature,
            transaction_hash: tx_hash,
        });

        tx.execute(&mut self.starknet.state, &self.starknet.block_context)?;

        Ok((tx_hash, contract_address))
    }

    fn add_account_transaction(&mut self, transaction: AccountTransaction) {
        self.starknet
            .handle_transaction(Transaction::AccountTransaction(transaction));
    }

    fn class_hash_at(
        &mut self,
        _block_id: BlockId,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, blockifier::state::errors::StateError> {
        self.starknet.state.get_class_hash_at(contract_address)
    }

    fn storage_at(
        &mut self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<StarkFelt, blockifier::state::errors::StateError> {
        self.starknet
            .state
            .get_storage_at(contract_address, storage_key)
    }

    fn chain_id(&self) -> ChainId {
        self.starknet.block_context.chain_id.clone()
    }

    fn block_number(&self) -> BlockNumber {
        self.starknet.block_context.block_number
    }

    fn block(&self, block_id: BlockId) -> Result<Block, blockifier::state::errors::StateError> {
        let block_number = match block_id {
            BlockId::Number(number) => BlockNumber(number),
            BlockId::Hash(hash) => *self
                .starknet
                .blocks
                .hash_to_num
                .get(&BlockHash(field_element_to_starkfelt(&hash)))
                .ok_or(blockifier::state::errors::StateError::StateReadError(
                    "block not found".to_string(),
                ))?,
            BlockId::Tag(tag) => {
                let current_height = self.starknet.blocks.current_height;
                match tag {
                    BlockTag::Latest => current_height.prev().unwrap(),
                    BlockTag::Pending => current_height,
                }
            }
        };

        let block = self.starknet.blocks.num_to_block.get(&block_number).ok_or(
            blockifier::state::errors::StateError::StateReadError("block not found".to_string()),
        )?;

        Ok(block.clone().0)
    }

    fn nonce_at(
        &mut self,
        _block_id: BlockId,
        contract_address: ContractAddress,
    ) -> Result<Nonce, blockifier::state::errors::StateError> {
        self.starknet.state.get_nonce_at(contract_address)
    }

    fn call(
        &self,
        _block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> Result<Vec<StarkFelt>> {
        let execution_info = self.starknet.call(function_call)?;
        Ok(execution_info.execution.retdata.0)
    }

    fn transaction(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::Transaction> {
        self.starknet.transactions.transaction(hash)
    }
}

pub trait Sequencer {
    fn chain_id(&self) -> ChainId;

    fn nonce_at(
        &mut self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> Result<Nonce, blockifier::state::errors::StateError>;

    fn block_number(&self) -> BlockNumber;

    fn block(&self, block_id: BlockId) -> Result<Block, blockifier::state::errors::StateError>;

    fn transaction(&self, hash: &TransactionHash)
        -> Option<starknet_api::transaction::Transaction>;

    fn class_hash_at(
        &mut self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, blockifier::state::errors::StateError>;

    fn call(
        &self,
        _block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> Result<Vec<StarkFelt>>;

    fn storage_at(
        &mut self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<StarkFelt, blockifier::state::errors::StateError>;

    fn deploy_account(
        &mut self,
        class_hash: ClassHash,
        version: TransactionVersion,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
    ) -> anyhow::Result<(TransactionHash, ContractAddress)>;

    fn add_account_transaction(&mut self, transaction: AccountTransaction);
}
