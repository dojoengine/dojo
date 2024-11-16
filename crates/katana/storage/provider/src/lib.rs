use std::collections::BTreeMap;
use std::ops::{Range, RangeInclusive};
use std::sync::Arc;

use katana_db::models::block::StoredBlockBodyIndices;
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockNumber, BlockWithTxHashes, FinalityStatus, Header,
    SealedBlockWithStatus,
};
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, ContractClass};
use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};
use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::Receipt;
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{TxHash, TxNumber, TxWithHash};
use katana_primitives::Felt;
use traits::block::{BlockIdReader, BlockStatusProvider, BlockWriter};
use traits::contract::{ContractClassProvider, ContractClassWriter, ContractClassWriterExt};
use traits::env::BlockEnvProvider;
use traits::state::{StateRootProvider, StateWriter};
use traits::transaction::{TransactionStatusProvider, TransactionTraceProvider};
use traits::trie::{ClassTrieWriter, ContractTrieWriter};

pub mod error;
pub mod providers;
pub mod traits;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

use crate::traits::block::{BlockHashProvider, BlockNumberProvider, BlockProvider, HeaderProvider};
use crate::traits::state::{StateFactoryProvider, StateProvider};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{ReceiptProvider, TransactionProvider, TransactionsProviderExt};

/// A result type for blockchain providers.
pub type ProviderResult<T> = Result<T, error::ProviderError>;

/// A blockchain provider that can be used to access the storage.
///
/// Serves as the main entrypoint for interacting with the storage storage. Every read/write
/// operation is done through this provider.
#[derive(Debug)]
pub struct BlockchainProvider<Db> {
    provider: Arc<Db>,
}

impl<Db> BlockchainProvider<Db> {
    pub fn new(provider: Db) -> Self {
        Self { provider: Arc::new(provider) }
    }
}

impl<Db> Clone for BlockchainProvider<Db> {
    fn clone(&self) -> Self {
        Self { provider: self.provider.clone() }
    }
}

impl<Db> BlockProvider for BlockchainProvider<Db>
where
    Db: BlockProvider,
{
    fn block(&self, id: BlockHashOrNumber) -> ProviderResult<Option<Block>> {
        self.provider.block(id)
    }

    fn block_with_tx_hashes(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BlockWithTxHashes>> {
        self.provider.block_with_tx_hashes(id)
    }

    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> ProviderResult<Vec<Block>> {
        self.provider.blocks_in_range(range)
    }

    fn block_body_indices(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<StoredBlockBodyIndices>> {
        self.provider.block_body_indices(id)
    }
}

impl<Db> HeaderProvider for BlockchainProvider<Db>
where
    Db: HeaderProvider,
{
    fn header(&self, id: BlockHashOrNumber) -> ProviderResult<Option<Header>> {
        self.provider.header(id)
    }
}

impl<Db> BlockNumberProvider for BlockchainProvider<Db>
where
    Db: BlockNumberProvider,
{
    fn latest_number(&self) -> ProviderResult<BlockNumber> {
        self.provider.latest_number()
    }

    fn block_number_by_hash(&self, hash: BlockHash) -> ProviderResult<Option<BlockNumber>> {
        self.provider.block_number_by_hash(hash)
    }
}

impl<Db> BlockHashProvider for BlockchainProvider<Db>
where
    Db: BlockHashProvider,
{
    fn latest_hash(&self) -> ProviderResult<BlockHash> {
        self.provider.latest_hash()
    }

    fn block_hash_by_num(&self, num: BlockNumber) -> ProviderResult<Option<BlockHash>> {
        self.provider.block_hash_by_num(num)
    }
}

impl<Db> BlockIdReader for BlockchainProvider<Db> where Db: BlockNumberProvider {}

impl<Db> BlockStatusProvider for BlockchainProvider<Db>
where
    Db: BlockStatusProvider,
{
    fn block_status(&self, id: BlockHashOrNumber) -> ProviderResult<Option<FinalityStatus>> {
        self.provider.block_status(id)
    }
}

impl<Db> BlockWriter for BlockchainProvider<Db>
where
    Db: BlockWriter,
{
    fn insert_block_with_states_and_receipts(
        &self,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithClasses,
        receipts: Vec<Receipt>,
        executions: Vec<TxExecInfo>,
    ) -> ProviderResult<()> {
        self.provider.insert_block_with_states_and_receipts(block, states, receipts, executions)
    }
}

impl<Db> TransactionProvider for BlockchainProvider<Db>
where
    Db: TransactionProvider,
{
    fn transaction_by_hash(&self, hash: TxHash) -> ProviderResult<Option<TxWithHash>> {
        self.provider.transaction_by_hash(hash)
    }

    fn transactions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<TxWithHash>>> {
        self.provider.transactions_by_block(block_id)
    }

    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> ProviderResult<Option<TxWithHash>> {
        self.provider.transaction_by_block_and_idx(block_id, idx)
    }

    fn transaction_count_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<u64>> {
        self.provider.transaction_count_by_block(block_id)
    }

    fn transaction_block_num_and_hash(
        &self,
        hash: TxHash,
    ) -> ProviderResult<Option<(BlockNumber, BlockHash)>> {
        TransactionProvider::transaction_block_num_and_hash(&self.provider, hash)
    }

    fn transaction_in_range(&self, range: Range<TxNumber>) -> ProviderResult<Vec<TxWithHash>> {
        self.provider.transaction_in_range(range)
    }
}

impl<Db> TransactionStatusProvider for BlockchainProvider<Db>
where
    Db: TransactionStatusProvider,
{
    fn transaction_status(&self, hash: TxHash) -> ProviderResult<Option<FinalityStatus>> {
        TransactionStatusProvider::transaction_status(&self.provider, hash)
    }
}

impl<Db> TransactionTraceProvider for BlockchainProvider<Db>
where
    Db: TransactionTraceProvider,
{
    fn transaction_execution(&self, hash: TxHash) -> ProviderResult<Option<TxExecInfo>> {
        TransactionTraceProvider::transaction_execution(&self.provider, hash)
    }

    fn transaction_executions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<TxExecInfo>>> {
        TransactionTraceProvider::transaction_executions_by_block(&self.provider, block_id)
    }

    fn transaction_executions_in_range(
        &self,
        range: Range<TxNumber>,
    ) -> ProviderResult<Vec<TxExecInfo>> {
        TransactionTraceProvider::transaction_executions_in_range(&self.provider, range)
    }
}

impl<Db> TransactionsProviderExt for BlockchainProvider<Db>
where
    Db: TransactionsProviderExt,
{
    fn transaction_hashes_in_range(&self, range: Range<TxNumber>) -> ProviderResult<Vec<TxHash>> {
        TransactionsProviderExt::transaction_hashes_in_range(&self.provider, range)
    }
}

impl<Db> ReceiptProvider for BlockchainProvider<Db>
where
    Db: ReceiptProvider,
{
    fn receipt_by_hash(&self, hash: TxHash) -> ProviderResult<Option<Receipt>> {
        self.provider.receipt_by_hash(hash)
    }

    fn receipts_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<Receipt>>> {
        self.provider.receipts_by_block(block_id)
    }
}

impl<Db> StateProvider for BlockchainProvider<Db>
where
    Db: StateProvider,
{
    fn nonce(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::contract::Nonce>> {
        self.provider.nonce(address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        self.provider.storage(address, storage_key)
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        self.provider.class_hash_of_contract(address)
    }
}

impl<Db> ContractClassProvider for BlockchainProvider<Db>
where
    Db: ContractClassProvider,
{
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<ContractClass>> {
        self.provider.class(hash)
    }

    fn compiled_class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        self.provider.compiled_class(hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        self.provider.compiled_class_hash_of_class_hash(hash)
    }
}

impl<Db> StateFactoryProvider for BlockchainProvider<Db>
where
    Db: StateFactoryProvider,
{
    fn latest(&self) -> ProviderResult<Box<dyn StateProvider>> {
        self.provider.latest()
    }

    fn historical(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Box<dyn StateProvider>>> {
        self.provider.historical(block_id)
    }
}

impl<Db> StateUpdateProvider for BlockchainProvider<Db>
where
    Db: StateUpdateProvider,
{
    fn state_update(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<StateUpdates>> {
        self.provider.state_update(block_id)
    }

    fn declared_classes(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BTreeMap<ClassHash, CompiledClassHash>>> {
        todo!()
    }

    fn deployed_contracts(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BTreeMap<ContractAddress, ClassHash>>> {
        todo!()
    }
}

impl<Db> StateRootProvider for BlockchainProvider<Db>
where
    Db: StateRootProvider,
{
    fn state_root(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<Felt>> {
        self.provider.state_root(block_id)
    }
}

impl<Db> ContractClassWriter for BlockchainProvider<Db>
where
    Db: ContractClassWriter,
{
    fn set_class(&self, hash: ClassHash, class: ContractClass) -> ProviderResult<()> {
        self.provider.set_class(hash, class)
    }

    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> ProviderResult<()> {
        self.provider.set_compiled_class_hash_of_class_hash(hash, compiled_hash)
    }
}

impl<Db> ContractClassWriterExt for BlockchainProvider<Db>
where
    Db: ContractClassWriterExt,
{
    fn set_compiled_class(&self, hash: ClassHash, class: CompiledClass) -> ProviderResult<()> {
        self.provider.set_compiled_class(hash, class)
    }
}

impl<Db> StateWriter for BlockchainProvider<Db>
where
    Db: StateWriter,
{
    fn set_storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
        storage_value: StorageValue,
    ) -> ProviderResult<()> {
        self.provider.set_storage(address, storage_key, storage_value)
    }

    fn set_class_hash_of_contract(
        &self,
        address: ContractAddress,
        class_hash: ClassHash,
    ) -> ProviderResult<()> {
        self.provider.set_class_hash_of_contract(address, class_hash)
    }

    fn set_nonce(
        &self,
        address: ContractAddress,
        nonce: katana_primitives::contract::Nonce,
    ) -> ProviderResult<()> {
        self.provider.set_nonce(address, nonce)
    }
}

impl<Db> BlockEnvProvider for BlockchainProvider<Db>
where
    Db: BlockEnvProvider,
{
    fn block_env_at(&self, id: BlockHashOrNumber) -> ProviderResult<Option<BlockEnv>> {
        self.provider.block_env_at(id)
    }
}

impl<Db> ClassTrieWriter for BlockchainProvider<Db>
where
    Db: ClassTrieWriter,
{
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> ProviderResult<Felt> {
        self.provider.insert_updates(block_number, updates)
    }
}

impl<Db> ContractTrieWriter for BlockchainProvider<Db>
where
    Db: ContractTrieWriter,
{
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        state_updates: &StateUpdates,
    ) -> ProviderResult<Felt> {
        self.provider.insert_updates(block_number, state_updates)
    }
}
