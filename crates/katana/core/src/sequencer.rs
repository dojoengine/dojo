use std::cmp::Ordering;
use std::iter::Skip;
use std::slice::Iter;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use auto_impl::auto_impl;
use blockifier::execution::contract_class::ContractClass;
use blockifier::state::state_api::StateReader;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::{DeclareTransaction, DeployAccountTransaction};
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, Event, EventsPage, FeeEstimate, FieldElement,
    FlattenedSierraClass, MaybePendingTransactionReceipt, StateUpdate,
};
use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;
use starknet_api::transaction::{InvokeTransaction, TransactionHash};
use tokio::time;

use crate::backend::config::StarknetConfig;
use crate::backend::contract::StarknetContract;
use crate::backend::state::{MemDb, StateExt};
use crate::backend::storage::block::ExecutedBlock;
use crate::backend::storage::transaction::{
    KnownTransaction, PendingTransaction, TransactionStatus,
};
use crate::backend::{Backend, ExternalFunctionCall};
use crate::sequencer_error::SequencerError;
use crate::utils::event::{ContinuationToken, ContinuationTokenError};

type SequencerResult<T> = Result<T, SequencerError>;

#[derive(Debug, Default)]
pub struct SequencerConfig {
    pub block_time: Option<u64>,
}

#[async_trait]
#[auto_impl(Arc)]
pub trait Sequencer {
    fn backend(&self) -> &Backend;

    async fn state(&self, block_id: &BlockId) -> SequencerResult<MemDb>;

    async fn chain_id(&self) -> ChainId;

    async fn transaction_receipt(
        &self,
        hash: &FieldElement,
    ) -> Option<MaybePendingTransactionReceipt>;

    async fn transaction_status(&self, hash: &FieldElement) -> Option<TransactionStatus>;

    async fn nonce_at(
        &self,
        block_id: BlockId,
        contract_address: ContractAddress,
    ) -> SequencerResult<Nonce>;

    async fn block_number(&self) -> u64;

    async fn block(&self, block_id: BlockId) -> Option<ExecutedBlock>;

    async fn transaction(&self, hash: &FieldElement) -> Option<KnownTransaction>;

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

    async fn block_hash_and_number(&self) -> (FieldElement, u64);

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
        address: Option<FieldElement>,
        keys: Option<Vec<Vec<FieldElement>>>,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> SequencerResult<EventsPage>;

    async fn state_update(&self, block_id: BlockId) -> SequencerResult<StateUpdate>;
}

pub struct KatanaSequencer {
    pub config: SequencerConfig,
    pub backend: Arc<Backend>,
}

impl KatanaSequencer {
    pub fn new(config: SequencerConfig, starknet_config: StarknetConfig) -> Self {
        Self { config, backend: Arc::new(Backend::new(starknet_config)) }
    }

    pub async fn start(&self) {
        // self.starknet.generate_genesis_block().await;

        if let Some(block_time) = self.config.block_time {
            let starknet = self.backend.clone();
            tokio::spawn(async move {
                loop {
                    starknet.generate_pending_block().await;
                    time::sleep(time::Duration::from_secs(block_time)).await;
                    starknet.generate_latest_block().await;
                }
            });
        } else {
            self.backend.generate_pending_block().await;
        }
    }

    // pub async fn drip_and_deploy_account(
    //     &self,
    //     transaction: DeployAccountTransaction,
    //     balance: u64,
    // ) -> SequencerResult<(TransactionHash, ContractAddress)> {
    //     let (transaction_hash, contract_address) =
    //         self.add_deploy_account_transaction(transaction).await;

    //     let deployed_account_balance_key =
    //         get_storage_var_address("ERC20_balances", &[*contract_address.0.key()])
    //             .map_err(SequencerError::StarknetApi)?;

    //     self.starknet.pending_cached_state.write().await.set_storage_at(
    //         self.starknet.block_context.read().fee_token_address,
    //         deployed_account_balance_key,
    //         stark_felt!(balance),
    //     );

    //     Ok((transaction_hash, contract_address))
    // }

    pub(self) async fn verify_contract_exists(&self, contract_address: &ContractAddress) -> bool {
        self.backend
            .state
            .write()
            .await
            .get_class_hash_at(*contract_address)
            .is_ok_and(|c| c != ClassHash::default())
    }
}

#[async_trait]
impl Sequencer for KatanaSequencer {
    fn backend(&self) -> &Backend {
        &self.backend
    }

    async fn state(&self, block_id: &BlockId) -> SequencerResult<MemDb> {
        match block_id {
            BlockId::Tag(BlockTag::Latest) => Ok(self.backend.state.read().await.clone()),

            BlockId::Tag(BlockTag::Pending) => {
                self.backend.pending_state().await.ok_or(SequencerError::StateNotFound(*block_id))
            }

            _ => {
                if let Some(hash) = self.backend.storage.read().await.block_hash(*block_id) {
                    self.backend
                        .states
                        .read()
                        .await
                        .get(&hash)
                        .cloned()
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
        let transaction_hash = transaction.tx.transaction_hash;
        let contract_address = transaction.contract_address;

        self.backend
            .handle_transaction(Transaction::AccountTransaction(AccountTransaction::DeployAccount(
                transaction,
            )))
            .await;

        (transaction_hash, contract_address)
    }

    async fn add_declare_transaction(
        &self,
        transaction: DeclareTransaction,
        sierra_class: Option<FlattenedSierraClass>,
    ) {
        let class_hash = transaction.tx().class_hash();

        self.backend
            .handle_transaction(Transaction::AccountTransaction(AccountTransaction::Declare(
                transaction,
            )))
            .await;

        if let Some(sierra_class) = sierra_class {
            self.backend
                .state
                .write()
                .await
                .classes
                .entry(class_hash)
                .and_modify(|r| r.sierra_class = Some(sierra_class));
        }
    }

    async fn add_invoke_transaction(&self, transaction: InvokeTransaction) {
        self.backend
            .handle_transaction(Transaction::AccountTransaction(AccountTransaction::Invoke(
                transaction,
            )))
            .await;
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
            AccountTransaction::Invoke(InvokeTransaction::V1(tx)) => tx.sender_address,
            AccountTransaction::Declare(tx) => tx.tx().sender_address(),
            AccountTransaction::DeployAccount(tx) => tx.contract_address,
            _ => return Err(SequencerError::UnsupportedTransaction),
        };

        let state = self.state(&block_id).await?;

        self.backend
            .estimate_fee(account_transaction, state)
            .map_err(SequencerError::TransactionExecution)
    }

    async fn block_hash_and_number(&self) -> (FieldElement, u64) {
        let hash = self.backend.storage.read().await.latest_hash;
        let number = self.backend.storage.read().await.latest_number;
        (hash, number)
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
        self.backend.block_context.read().chain_id.clone()
    }

    async fn block_number(&self) -> u64 {
        self.backend.storage.read().await.latest_number
    }

    async fn block(&self, block_id: BlockId) -> Option<ExecutedBlock> {
        match block_id {
            BlockId::Tag(BlockTag::Pending) => {
                self.backend.pending_block.read().await.as_ref().map(|b| b.as_block().into())
            }
            BlockId::Tag(BlockTag::Latest) => {
                let latest_hash = self.backend.storage.read().await.latest_hash;
                self.backend.storage.read().await.blocks.get(&latest_hash).map(|b| b.clone().into())
            }
            BlockId::Hash(hash) => {
                self.backend.storage.read().await.blocks.get(&hash).map(|b| b.clone().into())
            }
            BlockId::Number(num) => {
                let hash = *self.backend.storage.read().await.hashes.get(&num)?;
                self.backend.storage.read().await.blocks.get(&hash).map(|b| b.clone().into())
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

        self.backend
            .call(function_call, state)
            .map_err(SequencerError::EntryPointExecution)
            .map(|execution_info| execution_info.execution.retdata.0)
    }

    async fn transaction_status(&self, hash: &FieldElement) -> Option<TransactionStatus> {
        match self.backend.storage.read().await.transactions.get(hash) {
            Some(tx) => Some(tx.status()),
            // If the requested transaction is not available in the storage then
            // check if it is available in the pending block.
            None => self.backend.pending_block.read().await.as_ref().and_then(|b| {
                b.transactions
                    .iter()
                    .find(|tx| {
                        Into::<FieldElement>::into(tx.transaction.transaction_hash().0) == *hash
                    })
                    .map(|_| TransactionStatus::AcceptedOnL2)
            }),
        }
    }

    async fn transaction_receipt(
        &self,
        hash: &FieldElement,
    ) -> Option<MaybePendingTransactionReceipt> {
        let transaction = self.transaction(hash).await?;

        match transaction {
            KnownTransaction::Rejected(_) => None,
            KnownTransaction::Pending(tx) => {
                Some(MaybePendingTransactionReceipt::PendingReceipt(tx.receipt()))
            }
            KnownTransaction::Included(tx) => {
                Some(MaybePendingTransactionReceipt::Receipt(tx.receipt()))
            }
        }
    }

    async fn transaction(&self, hash: &FieldElement) -> Option<KnownTransaction> {
        match self.backend.storage.read().await.transactions.get(hash) {
            Some(tx) => Some(tx.clone()),
            // If the requested transaction is not available in the storage then
            // check if it is available in the pending block.
            None => self.backend.pending_block.read().await.as_ref().and_then(|b| {
                b.transactions
                    .iter()
                    .find(|tx| tx.hash() == *hash)
                    .map(|tx| PendingTransaction(tx.clone()).into())
            }),
        }
    }

    async fn events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
        address: Option<FieldElement>,
        keys: Option<Vec<Vec<FieldElement>>>,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> SequencerResult<EventsPage> {
        let mut current_block = 0;

        let (mut from_block, to_block) = {
            let storage = self.backend.storage.read().await;

            let from = storage
                .block_hash(from_block)
                .and_then(|hash| storage.blocks.get(&hash).map(|b| b.header.number))
                .ok_or(SequencerError::BlockNotFound(from_block))?;

            let to = storage
                .block_hash(to_block)
                .and_then(|hash| storage.blocks.get(&hash).map(|b| b.header.number))
                .ok_or(SequencerError::BlockNotFound(to_block))?;

            (from, to)
        };

        let mut continuation_token = match continuation_token {
            Some(token) => ContinuationToken::parse(token)?,
            None => ContinuationToken::default(),
        };

        // skip blocks that have been already read
        from_block += continuation_token.block_n;

        let mut filtered_events = Vec::with_capacity(chunk_size as usize);

        for i in from_block..=to_block {
            let block = self
                .backend
                .storage
                .read()
                .await
                .block_by_number(i)
                .cloned()
                .ok_or(SequencerError::BlockNotFound(BlockId::Number(i)))?;

            // to get the current block hash we need to get the parent hash of the next block
            // if the current block is the latest block then we use the latest hash
            let block_hash = self
                .backend
                .storage
                .read()
                .await
                .block_by_number(i + 1)
                .map(|b| b.header.parent_hash)
                .unwrap_or(self.backend.storage.read().await.latest_hash);

            let block_number = i;

            let txn_n = block.transactions.len();
            if (txn_n as u64) < continuation_token.txn_n {
                return Err(SequencerError::ContinuationToken(
                    ContinuationTokenError::InvalidToken,
                ));
            }

            for (txn_output, txn) in
                block.outputs.iter().zip(block.transactions).skip(continuation_token.txn_n as usize)
            {
                let txn_events_len: usize = txn_output.events.len();

                // check if continuation_token.event_n is correct
                match (txn_events_len as u64).cmp(&continuation_token.event_n) {
                    Ordering::Greater => (),
                    Ordering::Less => {
                        return Err(SequencerError::ContinuationToken(
                            ContinuationTokenError::InvalidToken,
                        ));
                    }
                    Ordering::Equal => {
                        continuation_token.txn_n += 1;
                        continuation_token.event_n = 0;
                        continue;
                    }
                }

                // skip events
                let txn_events = txn_output.events.iter().skip(continuation_token.event_n as usize);

                let (new_filtered_events, continuation_index) = filter_events_by_params(
                    txn_events,
                    address,
                    keys.clone(),
                    Some((chunk_size as usize) - filtered_events.len()),
                );

                filtered_events.extend(new_filtered_events.iter().map(|e| EmittedEvent {
                    from_address: e.from_address,
                    keys: e.keys.clone(),
                    data: e.data.clone(),
                    block_hash,
                    block_number,
                    transaction_hash: txn.hash(),
                }));

                if filtered_events.len() >= chunk_size as usize {
                    let token = if current_block < to_block
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
            .backend
            .storage
            .read()
            .await
            .block_hash(block_id)
            .ok_or(SequencerError::BlockNotFound(block_id))?;

        self.backend
            .storage
            .read()
            .await
            .state_update
            .get(&block_number)
            .cloned()
            .ok_or(SequencerError::StateUpdateNotFound(block_id))
    }
}

fn filter_events_by_params(
    events: Skip<Iter<'_, Event>>,
    address: Option<FieldElement>,
    filter_keys: Option<Vec<Vec<FieldElement>>>,
    max_results: Option<usize>,
) -> (Vec<Event>, usize) {
    let mut filtered_events = vec![];
    let mut index = 0;

    // Iterate on block events.
    for event in events {
        index += 1;
        if !address.map_or(true, |addr| addr == event.from_address) {
            continue;
        }

        let match_keys = match filter_keys {
            // From starknet-api spec:
            // Per key (by position), designate the possible values to be matched for events to be
            // returned. Empty array designates 'any' value"
            Some(ref filter_keys) => filter_keys.iter().enumerate().all(|(i, keys)| {
                // Lets say we want to filter events which are either named `Event1` or `Event2` and
                // custom key `0x1` or `0x2` Filter: [[sn_keccack("Event1"),
                // sn_keccack("Event2")], ["0x1", "0x2"]]

                // This checks: number of keys in event >= number of keys in filter (we check > i and not >= i because i is zero indexed)
                // because otherwise this event doesn't contain all the keys we requested
                event.keys.len() > i &&
                    // This checks: Empty array desginates 'any' value
                    (keys.is_empty()
                    ||
                    // This checks: If this events i'th value is one of the requested value in filter_keys[i]
                    keys.contains(&event.keys[i]))
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
