use std::collections::HashMap;
use std::sync::Arc;

use katana_db::models::block::StoredBlockBodyIndices;
use katana_primitives::block::{BlockHash, BlockNumber, Header, StateUpdate};
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, GenericContractInfo,
    SierraClass, StorageKey, StorageValue,
};
use katana_primitives::transaction::{Receipt, Tx, TxHash, TxNumber};
use parking_lot::RwLock;

type ContractStorageMap = HashMap<(ContractAddress, StorageKey), StorageValue>;
type ContractStateMap = HashMap<ContractAddress, GenericContractInfo>;

type SierraClassesMap = HashMap<ClassHash, SierraClass>;
type CompiledClassesMap = HashMap<ClassHash, CompiledContractClass>;
type CompiledClassHashesMap = HashMap<ClassHash, CompiledClassHash>;

#[derive(Default)]
pub struct SharedContractClasses {
    pub(crate) sierra_classes: RwLock<SierraClassesMap>,
    pub(crate) compiled_classes: RwLock<CompiledClassesMap>,
}

pub struct CacheSnapshotWithoutClasses<Db> {
    pub(crate) db: Db,
    pub(crate) storage: ContractStorageMap,
    pub(crate) contract_state: ContractStateMap,
    pub(crate) compiled_class_hashes: CompiledClassHashesMap,
}

pub struct CacheStateDb<Db> {
    pub(crate) db: Db,
    pub(crate) storage: RwLock<ContractStorageMap>,
    pub(crate) contract_state: RwLock<ContractStateMap>,
    pub(crate) shared_contract_classes: Arc<SharedContractClasses>,
    pub(crate) compiled_class_hashes: RwLock<CompiledClassHashesMap>,
}

pub struct CacheDb<Db> {
    pub(crate) db: Db,
    pub(crate) block_headers: HashMap<BlockNumber, Header>,
    pub(crate) block_hashes: HashMap<BlockNumber, BlockHash>,
    pub(crate) block_numbers: HashMap<BlockHash, BlockNumber>,
    pub(crate) block_body_indices: HashMap<BlockNumber, StoredBlockBodyIndices>,
    pub(crate) latest_block_hash: BlockHash,
    pub(crate) latest_block_number: BlockNumber,
    pub(crate) state_update: HashMap<BlockNumber, StateUpdate>,
    pub(crate) receipts: Vec<Receipt>,
    pub(crate) transactions: Vec<Tx>,
    pub(crate) transaction_hashes: HashMap<TxNumber, TxHash>,
    pub(crate) transaction_numbers: HashMap<TxHash, TxNumber>,
}

impl<Db> CacheStateDb<Db> {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            storage: RwLock::new(HashMap::new()),
            contract_state: RwLock::new(HashMap::new()),
            compiled_class_hashes: RwLock::new(HashMap::new()),
            shared_contract_classes: Arc::new(SharedContractClasses::default()),
        }
    }
}

impl<Db> CacheDb<Db> {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            receipts: Vec::new(),
            transactions: Vec::new(),
            state_update: HashMap::new(),
            block_hashes: HashMap::new(),
            block_headers: HashMap::new(),
            block_numbers: HashMap::new(),
            transaction_hashes: HashMap::new(),
            block_body_indices: HashMap::new(),
            transaction_numbers: HashMap::new(),
            latest_block_hash: Default::default(),
            latest_block_number: Default::default(),
        }
    }
}

impl<Db> std::ops::Deref for CacheStateDb<Db> {
    type Target = Db;
    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl<Db> std::ops::Deref for CacheDb<Db> {
    type Target = Db;
    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl<Db: Clone> CacheStateDb<Db> {
    pub fn create_snapshot_without_classes(&self) -> CacheSnapshotWithoutClasses<Db> {
        CacheSnapshotWithoutClasses {
            db: self.db.clone(),
            storage: self.storage.read().clone(),
            contract_state: self.contract_state.read().clone(),
            compiled_class_hashes: self.compiled_class_hashes.read().clone(),
        }
    }
}
