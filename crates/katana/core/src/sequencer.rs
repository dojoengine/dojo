use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::execution::contract_class::ContractClass;
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use starknet::core::types::{BlockId, BlockTag, FeeEstimate, StateUpdate, TransactionStatus};
use starknet_api::block::{BlockHash, BlockNumber, BlockTimestamp};
use starknet_api::core::{calculate_contract_address, ChainId, ClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeployAccountTransaction, Fee,
    Transaction as StarknetApiTransaction, TransactionHash, TransactionSignature,
};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::time;

use crate::accounts::Account;
use crate::sequencer_error::SequencerError;
use crate::starknet::block::StarknetBlock;
use crate::starknet::event::EmittedEvent;
use crate::starknet::transaction::ExternalFunctionCall;
use crate::starknet::{StarknetConfig, StarknetWrapper};
use crate::state::DictStateReader;
use crate::util::starkfelt_to_u128;

type SequencerResult<T> = Result<T, SequencerError>;

#[derive(Debug, Default)]
pub struct SequencerConfig {
    pub block_time: Option<u64>,
}

pub struct KatanaSequencer {
    pub config: SequencerConfig,
    pub starknet: Arc<RwLock<StarknetWrapper>>,
}

impl KatanaSequencer {
    pub fn new(config: SequencerConfig, starknet_config: StarknetConfig) -> Self {
        Self { config, starknet: Arc::new(RwLock::new(StarknetWrapper::new(starknet_config))) }
    }

    pub async fn start(&self) {
        self.starknet.write().await.generate_genesis_block();

        if let Some(block_time) = self.config.block_time {
            let starknet = self.starknet.clone();
            tokio::spawn(async move {
                loop {
                    starknet.write().await.generate_pending_block();
                    time::sleep(time::Duration::from_secs(block_time)).await;
                    starknet.write().await.generate_latest_block();
                }
            });
        } else {
            self.starknet.write().await.generate_pending_block();
        }
    }

    pub async fn drip_and_deploy_account(
        &self,
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

        self.starknet.write().await.pending_state.set_storage_at(
            self.starknet.read().await.block_context.fee_token_address,
            deployed_account_balance_key,
            stark_felt!(balance),
        );

        self.deploy_account(class_hash, contract_address_salt, constructor_calldata, signature)
            .await
    }

    pub async fn block_number_from_block_id(&self, block_id: &BlockId) -> Option<BlockNumber> {
        match block_id {
            BlockId::Number(number) => Some(BlockNumber(*number)),

            BlockId::Hash(hash) => self
                .starknet
                .read()
                .await
                .blocks
                .hash_to_num
                .get(&BlockHash(StarkFelt::from(*hash)))
                .cloned(),

            BlockId::Tag(BlockTag::Pending) => None,
            BlockId::Tag(BlockTag::Latest) => {
                self.starknet.write().await.blocks.current_block_number()
            }
        }
    }

    pub(self) async fn verify_contract_exists(&self, contract_address: &ContractAddress) -> bool {
        self.starknet.write().await.state.address_to_class_hash.contains_key(contract_address)
    }
}

#[async_trait]
impl Sequencer for KatanaSequencer {
    async fn starknet(&self) -> RwLockReadGuard<'_, StarknetWrapper> {
        self.starknet.read().await
    }

    async fn mut_starknet(&self) -> RwLockWriteGuard<'_, StarknetWrapper> {
        self.starknet.write().await
    }

    async fn state(&self, block_id: &BlockId) -> SequencerResult<DictStateReader> {
        match block_id {
            BlockId::Tag(BlockTag::Latest) => Ok(self.starknet.write().await.latest_state()),
            BlockId::Tag(BlockTag::Pending) => Ok(self.starknet.write().await.pending_state()),
            _ => {
                if let Some(number) = self.block_number_from_block_id(block_id).await {
                    self.starknet
                        .read()
                        .await
                        .state(number)
                        .ok_or(SequencerError::StateNotFound(*block_id))
                } else {
                    Err(SequencerError::BlockNotFound(*block_id))
                }
            }
        }
    }

    async fn deploy_account(
        &self,
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
                .write()
                .await
                .state
                .get_storage_at(
                    self.starknet.read().await.block_context.fee_token_address,
                    account_balance_key,
                )
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

        tx.execute(
            &mut self.starknet.write().await.pending_state,
            &self.starknet.read().await.block_context,
        )
        .map_err(SequencerError::TransactionExecution)?;

        Ok((tx_hash, contract_address))
    }

    async fn add_account_transaction(&self, transaction: AccountTransaction) {
        self.starknet
            .write()
            .await
            .handle_transaction(Transaction::AccountTransaction(transaction));
    }

    async fn estimate_fee(
        &self,
        account_transaction: AccountTransaction,
        block_id: BlockId,
    ) -> SequencerResult<FeeEstimate> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        let sender = match &account_transaction {
            AccountTransaction::Invoke(tx) => tx.sender_address(),
            AccountTransaction::Declare(tx) => tx.tx().sender_address(),
            AccountTransaction::DeployAccount(tx) => tx.contract_address,
        };

        if !self.verify_contract_exists(&sender).await {
            return Err(SequencerError::ContractNotFound(sender));
        }

        let state = self.state(&block_id).await?;

        let exec_info = self
            .starknet
            .write()
            .await
            .simulate_transaction(account_transaction, Some(state))
            .map_err(SequencerError::TransactionExecution)?;

        let (l1_gas_usage, vm_resources) = extract_l1_gas_and_vm_usage(&exec_info.actual_resources);
        let l1_gas_by_vm_usage =
            calculate_l1_gas_by_vm_usage(&self.starknet.read().await.block_context, &vm_resources)
                .map_err(SequencerError::TransactionExecution)?;

        let total_l1_gas_usage = l1_gas_usage as f64 + l1_gas_by_vm_usage;

        Ok(FeeEstimate {
            overall_fee: total_l1_gas_usage.ceil() as u64
                * self.starknet.read().await.block_context.gas_price as u64,
            gas_consumed: total_l1_gas_usage.ceil() as u64,
            gas_price: self.starknet.read().await.block_context.gas_price as u64,
        })
    }

    async fn block_hash_and_number(&self) -> Option<(BlockHash, BlockNumber)> {
        let block = self.starknet.read().await.blocks.latest()?;
        Some((block.block_hash(), block.block_number()))
    }

    async fn class_hash_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<ClassHash> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        if !self.verify_contract_exists(&contract_address).await {
            return Err(SequencerError::ContractNotFound(contract_address));
        }

        let mut state = self.state(&block_id).await?;
        state.get_class_hash_at(contract_address).map_err(SequencerError::State)
    }

    async fn class(
        &self,
        block_id: BlockId,
        class_hash: ClassHash,
    ) -> SequencerResult<ContractClass> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        let mut state = self.state(&block_id).await?;
        state.get_compiled_contract_class(&class_hash).map_err(SequencerError::State)
    }

    async fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockId,
    ) -> SequencerResult<StarkFelt> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        if !self.verify_contract_exists(&contract_address).await {
            return Err(SequencerError::ContractNotFound(contract_address));
        }

        let mut state = self.state(&block_id).await?;
        state.get_storage_at(contract_address, storage_key).map_err(SequencerError::State)
    }

    async fn chain_id(&self) -> ChainId {
        self.starknet.read().await.block_context.chain_id.clone()
    }

    async fn block_number(&self) -> BlockNumber {
        self.starknet.read().await.block_context.block_number
    }

    async fn next_block_timestamp(&self) -> BlockTimestamp {
        self.starknet.read().await.block_context.block_timestamp
    }

    async fn block(&self, block_id: BlockId) -> Option<StarknetBlock> {
        match block_id {
            BlockId::Tag(BlockTag::Pending) => {
                self.starknet.read().await.blocks.pending_block.clone()
            }
            _ => {
                if let Some(number) = self.block_number_from_block_id(&block_id).await {
                    self.starknet.read().await.blocks.by_number(number)
                } else {
                    None
                }
            }
        }
    }

    async fn nonce_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<Nonce> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        if !self.verify_contract_exists(&contract_address).await {
            return Err(SequencerError::ContractNotFound(contract_address));
        }

        let mut state = self.state(&block_id).await?;
        state.get_nonce_at(contract_address).map_err(SequencerError::State)
    }

    async fn call(
        &self,
        block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> SequencerResult<Vec<StarkFelt>> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        if !self.verify_contract_exists(&function_call.contract_address).await {
            return Err(SequencerError::ContractNotFound(function_call.contract_address));
        }

        let state = self.state(&block_id).await?;

        self.starknet
            .write()
            .await
            .call(function_call, Some(state))
            .map_err(SequencerError::EntryPointExecution)
            .map(|execution_info| execution_info.execution.retdata.0)
    }

    async fn transaction_status(&self, hash: &TransactionHash) -> Option<TransactionStatus> {
        self.starknet.read().await.transactions.by_hash(hash).map(|tx| tx.status)
    }

    async fn transaction_receipt(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::TransactionReceipt> {
        self.starknet.read().await.transactions.by_hash(hash).map(|tx| tx.receipt())
    }

    async fn transaction(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::Transaction> {
        self.starknet.read().await.transactions.by_hash(hash).map(|tx| tx.inner.clone())
    }

    async fn events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
        address: Option<StarkFelt>,
        keys: Option<Vec<Vec<StarkFelt>>>,
        _continuation_token: Option<String>,
        _chunk_size: u64,
    ) -> SequencerResult<Vec<EmittedEvent>> {
        let from_block = self
            .block_number_from_block_id(&from_block)
            .await
            .ok_or(SequencerError::BlockNotFound(from_block))?;

        let to_block = self
            .block_number_from_block_id(&to_block)
            .await
            .ok_or(SequencerError::BlockNotFound(to_block))?;

        let mut events = Vec::new();
        for i in from_block.0..to_block.0 {
            let block = self
                .starknet
                .read()
                .await
                .blocks
                .by_number(BlockNumber(i))
                .ok_or(SequencerError::BlockNotFound(BlockId::Number(i)))?;

            for tx in block.transactions() {
                match tx {
                    StarknetApiTransaction::Invoke(_) | StarknetApiTransaction::L1Handler(_) => {}
                    _ => continue,
                }

                let sn = self.starknet.read().await;
                let sn_tx = sn
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

    async fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate> {
        let block_number = self
            .block_number_from_block_id(&block_id)
            .await
            .ok_or(SequencerError::BlockNotFound(block_id))?;

        self.starknet
            .read()
            .await
            .blocks
            .get_state_update(block_number)
            .ok_or(SequencerError::StateUpdateNotFound(block_id))
    }

    async fn generate_new_block(&self) {
        self.starknet.write().await.generate_latest_block();
        self.starknet.write().await.generate_pending_block();
    }

    async fn set_next_block_timestamp(&self, timestamp: u64) -> SequencerResult<()> {
        self.starknet.write().await.set_next_block_timestamp(timestamp)
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> SequencerResult<()> {
        self.starknet.write().await.increase_next_block_timestamp(timestamp)
    }

    async fn predeployed_accounts(&self) -> Vec<Account> {
        self.starknet.read().await.predeployed_accounts.accounts.clone()
    }
}

#[async_trait]
pub trait Sequencer {
    async fn starknet(&self) -> RwLockReadGuard<'_, StarknetWrapper>;

    async fn mut_starknet(&self) -> RwLockWriteGuard<'_, StarknetWrapper>;

    async fn state(&self, block_id: &BlockId) -> SequencerResult<DictStateReader>;

    async fn chain_id(&self) -> ChainId;

    async fn generate_new_block(&self);

    async fn predeployed_accounts(&self) -> Vec<Account>;

    async fn transaction_receipt(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::TransactionReceipt>;

    async fn transaction_status(&self, hash: &TransactionHash) -> Option<TransactionStatus>;

    async fn nonce_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<Nonce>;

    async fn block_number(&self) -> BlockNumber;

    async fn next_block_timestamp(&self) -> BlockTimestamp;

    async fn block(&self, block_id: BlockId) -> Option<StarknetBlock>;

    async fn transaction(
        &self,
        hash: &TransactionHash,
    ) -> Option<starknet_api::transaction::Transaction>;

    async fn class_hash_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<ClassHash>;

    async fn class(
        &self,
        block_id: BlockId,
        class_hash: ClassHash,
    ) -> SequencerResult<ContractClass>;

    async fn block_hash_and_number(&self) -> Option<(BlockHash, BlockNumber)>;

    async fn call(
        &self,
        block_id: BlockId,
        function_call: ExternalFunctionCall,
    ) -> SequencerResult<Vec<StarkFelt>>;

    async fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockId,
    ) -> SequencerResult<StarkFelt>;

    async fn deploy_account(
        &self,
        class_hash: ClassHash,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
    ) -> SequencerResult<(TransactionHash, ContractAddress)>;

    async fn add_account_transaction(&self, transaction: AccountTransaction);

    async fn estimate_fee(
        &self,
        account_transaction: AccountTransaction,
        block_id: BlockId,
    ) -> SequencerResult<FeeEstimate>;

    async fn events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
        address: Option<StarkFelt>,
        keys: Option<Vec<Vec<StarkFelt>>>,
        _continuation_token: Option<String>,
        _chunk_size: u64,
    ) -> SequencerResult<Vec<EmittedEvent>>;

    async fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate>;

    async fn set_next_block_timestamp(&self, timestamp: u64) -> SequencerResult<()>;

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> SequencerResult<()>;
}
