use std::cmp::Ordering;
use std::iter::Skip;
use std::slice::Iter;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use auto_impl::auto_impl;
use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::execution::contract_class::ContractClass;
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::DeclareTransaction;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, EventsPage, FeeEstimate, FlattenedSierraClass, StateUpdate,
    TransactionStatus,
};
use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    DeployAccountTransaction, Event, InvokeTransaction, TransactionHash,
};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::time;

use crate::backend::block::StarknetBlock;
use crate::backend::config::StarknetConfig;
use crate::backend::contract::StarknetContract;
use crate::backend::state::{MemDb, StateExt};
use crate::backend::transaction::ExternalFunctionCall;
use crate::backend::StarknetWrapper;
use crate::sequencer_error::SequencerError;
use crate::util::{ContinuationToken, ContinuationTokenError};

type SequencerResult<T> = Result<T, SequencerError>;

#[derive(Debug, Default)]
pub struct SequencerConfig {
    pub block_time: Option<u64>,
}

#[async_trait]
#[auto_impl(Arc)]
pub trait Sequencer {
    async fn starknet(&self) -> RwLockReadGuard<'_, StarknetWrapper>;

    async fn mut_starknet(&self) -> RwLockWriteGuard<'_, StarknetWrapper>;

    async fn state(&self, block_id: &BlockId) -> SequencerResult<MemDb>;

    async fn chain_id(&self) -> ChainId;

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
    ) -> SequencerResult<StarknetContract>;

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

    async fn add_deploy_account_transaction(
        &self,
        transaction: DeployAccountTransaction,
    ) -> (TransactionHash, ContractAddress);

    async fn add_declare_transaction(
        &self,
        transaction: DeclareTransaction,
        sierra_class: Option<FlattenedSierraClass>,
    );

    async fn add_invoke_transaction(&self, transaction: InvokeTransaction);

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
    ) -> SequencerResult<EventsPage>;

    async fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate>;
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
        transaction: DeployAccountTransaction,
        balance: u64,
    ) -> SequencerResult<(TransactionHash, ContractAddress)> {
        let (transaction_hash, contract_address) =
            self.add_deploy_account_transaction(transaction).await;

        let deployed_account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()])
                .map_err(SequencerError::StarknetApi)?;

        self.starknet.write().await.pending_cached_state.set_storage_at(
            self.starknet.read().await.block_context.fee_token_address,
            deployed_account_balance_key,
            stark_felt!(balance),
        );

        Ok((transaction_hash, contract_address))
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
                Some(self.starknet.write().await.blocks.current_block_number())
            }
        }
    }

    pub(self) async fn verify_contract_exists(&self, contract_address: &ContractAddress) -> bool {
        self.starknet
            .write()
            .await
            .state
            .get_class_hash_at(*contract_address)
            .is_ok_and(|c| c != ClassHash::default())
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

    async fn state(&self, block_id: &BlockId) -> SequencerResult<MemDb> {
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

    async fn add_deploy_account_transaction(
        &self,
        transaction: DeployAccountTransaction,
    ) -> (TransactionHash, ContractAddress) {
        let transaction_hash = transaction.transaction_hash;
        let contract_address = transaction.contract_address;

        self.starknet.write().await.handle_transaction(Transaction::AccountTransaction(
            AccountTransaction::DeployAccount(transaction),
        ));

        (transaction_hash, contract_address)
    }

    async fn add_declare_transaction(
        &self,
        transaction: DeclareTransaction,
        sierra_class: Option<FlattenedSierraClass>,
    ) {
        let class_hash = transaction.tx().class_hash();

        self.starknet.write().await.handle_transaction(Transaction::AccountTransaction(
            AccountTransaction::Declare(transaction),
        ));

        if let Some(sierra_class) = sierra_class {
            self.starknet
                .write()
                .await
                .state
                .classes
                .entry(class_hash)
                .and_modify(|r| r.sierra_class = Some(sierra_class));
        }
    }

    async fn add_invoke_transaction(&self, transaction: InvokeTransaction) {
        self.starknet.write().await.handle_transaction(Transaction::AccountTransaction(
            AccountTransaction::Invoke(transaction),
        ));
    }

    async fn estimate_fee(
        &self,
        account_transaction: AccountTransaction,
        block_id: BlockId,
    ) -> SequencerResult<FeeEstimate> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        match &account_transaction {
            AccountTransaction::Invoke(tx) => tx.sender_address(),
            AccountTransaction::Declare(tx) => tx.tx().sender_address(),
            AccountTransaction::DeployAccount(tx) => tx.contract_address,
        };

        let state = self.state(&block_id).await?;

        self.starknet
            .write()
            .await
            .estimate_fee(account_transaction, Some(state))
            .map_err(SequencerError::TransactionExecution)
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
    ) -> SequencerResult<StarknetContract> {
        if self.block(block_id).await.is_none() {
            return Err(SequencerError::BlockNotFound(block_id));
        }

        let mut state = self.state(&block_id).await?;

        match state.get_compiled_contract_class(&class_hash).map_err(SequencerError::State)? {
            ContractClass::V0(c) => Ok(StarknetContract::Legacy(c)),
            ContractClass::V1(_) => state
                .get_sierra_class(&class_hash)
                .map(StarknetContract::Sierra)
                .map_err(SequencerError::State),
        }
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
        self.starknet.read().await.blocks.current_block_number()
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
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> SequencerResult<EventsPage> {
        let mut current_block = 0;
        let mut from_block = self
            .block_number_from_block_id(&from_block)
            .await
            .ok_or(SequencerError::BlockNotFound(from_block))?;

        let to_block = self
            .block_number_from_block_id(&to_block)
            .await
            .ok_or(SequencerError::BlockNotFound(to_block))?;

        let mut continuation_token = ContinuationToken::parse(continuation_token)?;
        // skip blocks that have been already read
        from_block.0 += continuation_token.block_n;

        let mut filtered_events = Vec::with_capacity(chunk_size as usize);

        for i in from_block.0..=to_block.0 {
            let block = self
                .starknet
                .read()
                .await
                .blocks
                .by_number(BlockNumber(i))
                .ok_or(SequencerError::BlockNotFound(BlockId::Number(i)))?;
            let block_hash = block.block_hash().0.into();
            let block_number = i;

            let txn_n = block.transactions().len();
            if (txn_n as u64) < continuation_token.txn_n {
                return Err(SequencerError::ContinuationToken(
                    ContinuationTokenError::InvalidToken,
                ));
            }

            for (txn_output, txn) in block
                .transaction_outputs()
                .iter()
                .zip(block.transactions())
                .skip(continuation_token.txn_n as usize)
            {
                let txn_events_len: usize = txn_output.events().len();

                // check if continuation_token.event_n is correct
                match (txn_events_len as u64).cmp(&continuation_token.event_n) {
                    Ordering::Greater => (),
                    Ordering::Less => {
                        return Err(SequencerError::ContinuationToken(
                            ContinuationTokenError::InvalidToken,
                        ))
                    }
                    Ordering::Equal => {
                        continuation_token.txn_n += 1;
                        continuation_token.event_n = 0;
                        continue;
                    }
                }

                // skip events
                let txn_events =
                    txn_output.events().iter().skip(continuation_token.event_n as usize);

                let (new_filtered_events, continuation_index) = filter_events_by_params(
                    txn_events,
                    address,
                    keys.clone(),
                    Some((chunk_size as usize) - filtered_events.len()),
                );

                filtered_events.extend(new_filtered_events.iter().map(|e| EmittedEvent {
                    from_address: (*e.from_address.0.key()).into(),
                    keys: e.content.keys.clone().into_iter().map(|key| key.0.into()).collect(),
                    data: e.content.data.0.clone().into_iter().map(|data| data.into()).collect(),
                    block_hash,
                    block_number,
                    transaction_hash: txn.transaction_hash().0.into(),
                }));

                if filtered_events.len() >= chunk_size as usize {
                    let token = if current_block < to_block.0
                        || continuation_token.txn_n < txn_n as u64 - 1
                        || continuation_index < txn_events_len
                    {
                        continuation_token.event_n = continuation_index as u64;
                        Some(continuation_token.to_string())
                    } else {
                        None
                    };
                    return Ok(EventsPage { events: filtered_events, continuation_token: token });
                }

                continuation_token.txn_n += 1;
                continuation_token.event_n = 0;
            }

            current_block += 1;
            continuation_token.block_n += 1;
            continuation_token.txn_n = 0;
        }

        Ok(EventsPage { events: filtered_events, continuation_token: None })
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
}

fn filter_events_by_params(
    events: Skip<Iter<'_, Event>>,
    address: Option<StarkFelt>,
    filter_keys: Option<Vec<Vec<StarkFelt>>>,
    max_results: Option<usize>,
) -> (Vec<Event>, usize) {
    let mut filtered_events = vec![];
    let mut index = 0;

    // Iterate on block events.
    for event in events {
        index += 1;
        if !address.map_or(true, |addr| addr == *event.from_address.0.key()) {
            continue;
        }

        let match_keys = match filter_keys {
            // From starknet-api spec:
            // Per key (by position), designate the possible values to be matched for events to be returned. Empty array designates 'any' value"
            Some(ref filter_keys) => filter_keys.iter().enumerate().all(|(i, keys)| {
                // Lets say we want to filter events which are either named `Event1` or `Event2` and custom key `0x1` or `0x2`
                // Filter: [[sn_keccack("Event1"), sn_keccack("Event2")], ["0x1", "0x2"]]

                // This checks: number of keys in event >= number of keys in filter (we check > i and not >= i because i is zero indexed)
                // because otherwise this event doesn't contain all the keys we requested
                event.content.keys.len() > i &&
                    // This checks: Empty array desginates 'any' value
                    (keys.is_empty()
                    ||
                    // This checks: If this events i'th value is one of the requested value in filter_keys[i]
                    keys.contains(&event.content.keys[i].0))
            }),
            None => true,
        };

        if match_keys {
            filtered_events.push(event.clone());
            if let Some(max_results) = max_results {
                if filtered_events.len() >= max_results {
                    break;
                }
            }
        }
    }
    (filtered_events, index)
}
