use std::ops::{Range, RangeInclusive};

use anyhow::Result;
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockNumber, Header, StateUpdate,
};
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, GenericContractInfo,
    SierraClass, StorageKey, StorageValue,
};
use katana_primitives::transaction::{Receipt, Tx, TxHash, TxNumber};

pub mod providers;
pub mod traits;

use crate::traits::block::{BlockHashProvider, BlockNumberProvider, BlockProvider, HeaderProvider};
use crate::traits::contract::ContractProvider;
use crate::traits::state::{StateFactoryProvider, StateProvider, StateProviderExt};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{ReceiptProvider, TransactionProvider, TransactionsProviderExt};

/// A blockchain provider that can be used to access the storage.
///
/// Serves as the main entrypoint for interacting with the storage storage. Every read/write
/// operation is done through this provider.
pub struct BlockchainProvider<Db> {
    provider: Db,
}

impl<Db> BlockchainProvider<Db> {
    pub fn new(provider: Db) -> Self {
        Self { provider }
    }
}

impl<Db> BlockProvider for BlockchainProvider<Db>
where
    Db: BlockProvider,
{
    fn block(&self, id: BlockHashOrNumber) -> Result<Option<Block>> {
        self.provider.block(id)
    }

    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> Result<Vec<Block>> {
        self.provider.blocks_in_range(range)
    }
}

impl<Db> HeaderProvider for BlockchainProvider<Db>
where
    Db: HeaderProvider,
{
    fn header(&self, id: BlockHashOrNumber) -> Result<Option<Header>> {
        self.provider.header(id)
    }
}

impl<Db> BlockNumberProvider for BlockchainProvider<Db>
where
    Db: BlockNumberProvider,
{
    fn latest_number(&self) -> Result<BlockNumber> {
        self.provider.latest_number()
    }

    fn block_number_by_hash(&self, hash: BlockHash) -> Result<Option<BlockNumber>> {
        self.provider.block_number_by_hash(hash)
    }
}

impl<Db> BlockHashProvider for BlockchainProvider<Db>
where
    Db: BlockHashProvider,
{
    fn latest_hash(&self) -> Result<BlockHash> {
        self.provider.latest_hash()
    }

    fn block_hash_by_num(&self, num: BlockNumber) -> Result<Option<BlockHash>> {
        self.provider.block_hash_by_num(num)
    }
}

impl<Db> TransactionProvider for BlockchainProvider<Db>
where
    Db: TransactionProvider,
{
    fn transaction_by_hash(&self, hash: TxHash) -> Result<Option<Tx>> {
        self.provider.transaction_by_hash(hash)
    }

    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> Result<Option<Tx>> {
        self.provider.transaction_by_block_and_idx(block_id, idx)
    }

    fn transactions_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Tx>>> {
        self.provider.transactions_by_block(block_id)
    }
}

impl<Db> TransactionsProviderExt for BlockchainProvider<Db>
where
    Db: TransactionsProviderExt,
{
    fn transaction_hashes_by_range(&self, range: Range<TxNumber>) -> Result<Vec<TxHash>> {
        self.provider.transaction_hashes_by_range(range)
    }
}

impl<Db> ReceiptProvider for BlockchainProvider<Db>
where
    Db: ReceiptProvider,
{
    fn receipt_by_hash(&self, hash: TxHash) -> Result<Option<Receipt>> {
        self.provider.receipt_by_hash(hash)
    }

    fn receipts_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Receipt>>> {
        self.provider.receipts_by_block(block_id)
    }
}

impl<Db> StateProvider for BlockchainProvider<Db>
where
    Db: StateProvider,
{
    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        self.provider.class(hash)
    }

    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
        self.provider.class_hash_of_contract(address)
    }

    fn nonce(
        &self,
        address: ContractAddress,
    ) -> Result<Option<katana_primitives::contract::Nonce>> {
        self.provider.nonce(address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        self.provider.storage(address, storage_key)
    }
}

impl<Db> StateProviderExt for BlockchainProvider<Db>
where
    Db: StateProviderExt,
{
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<SierraClass>> {
        self.provider.sierra_class(hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>> {
        self.provider.compiled_class_hash_of_class_hash(hash)
    }
}

impl<Db> StateFactoryProvider for BlockchainProvider<Db>
where
    Db: StateFactoryProvider,
{
    fn latest(&self) -> Result<Box<dyn StateProvider>> {
        self.provider.latest()
    }

    fn historical(&self, block_id: BlockHashOrNumber) -> Result<Option<Box<dyn StateProvider>>> {
        self.provider.historical(block_id)
    }
}

impl<Db> StateUpdateProvider for BlockchainProvider<Db>
where
    Db: StateUpdateProvider,
{
    fn state_update(&self, block_id: BlockHashOrNumber) -> Result<Option<StateUpdate>> {
        self.provider.state_update(block_id)
    }
}

impl<Db> ContractProvider for BlockchainProvider<Db>
where
    Db: ContractProvider,
{
    fn contract(&self, address: ContractAddress) -> Result<Option<GenericContractInfo>> {
        self.provider.contract(address)
    }
}
