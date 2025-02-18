use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash, ContractClass};
use katana_primitives::contract::{Nonce, StorageKey, StorageValue};
use katana_primitives::{ContractAddress, Felt};
use katana_trie::MultiProof;

use super::cache::{CacheSnapshotWithoutClasses, CacheStateDb, SharedContractClasses};
use crate::traits::contract::ContractClassProvider;
use crate::traits::state::{StateProofProvider, StateProvider, StateRootProvider};
use crate::ProviderResult;

#[derive(Debug)]
pub struct StateSnapshot<Db> {
    // because the classes are shared between snapshots, when trying to fetch check the compiled
    // hash first and then the sierra class to ensure the class should be present in the snapshot.
    pub(crate) classes: Arc<SharedContractClasses>,
    pub(crate) inner: CacheSnapshotWithoutClasses<Db>,
}

const DEFAULT_HISTORY_LIMIT: usize = 500;
const MIN_HISTORY_LIMIT: usize = 10;

/// Represents the complete state of a single block.
///
/// It should store at N - 1 states, where N is the latest block number.
#[derive(Debug)]
pub struct HistoricalStates {
    /// The states at a certain block based on the block number
    states: HashMap<BlockNumber, Arc<dyn StateProvider>>,
    /// How many states to store at most
    in_memory_limit: usize,
    /// minimum amount of states we keep in memory
    min_in_memory_limit: usize,
    /// all states present, used to enforce `in_memory_limit`
    present: VecDeque<BlockNumber>,
}

impl HistoricalStates {
    pub fn new(limit: usize) -> Self {
        Self {
            in_memory_limit: limit,
            states: Default::default(),
            present: Default::default(),
            min_in_memory_limit: limit.min(MIN_HISTORY_LIMIT),
        }
    }

    /// Returns the state for the given `block_hash` if present
    pub fn get(&self, block_num: &BlockNumber) -> Option<&Arc<dyn StateProvider>> {
        self.states.get(block_num)
    }

    /// Inserts a new (block_hash -> state) pair
    ///
    /// When the configured limit for the number of states that can be stored in memory is reached,
    /// the oldest state is removed.
    ///
    /// Since we keep a snapshot of the entire state as history, the size of the state will increase
    /// with the transactions processed. To counter this, we gradually decrease the cache limit with
    /// the number of states/blocks until we reached the `min_limit`.
    pub fn insert(&mut self, block_num: BlockNumber, state: Box<dyn StateProvider>) {
        if self.present.len() >= self.in_memory_limit {
            // once we hit the max limit we gradually decrease it
            self.in_memory_limit =
                self.in_memory_limit.saturating_sub(1).max(self.min_in_memory_limit);
        }

        self.enforce_limits();
        self.states.insert(block_num, Arc::new(state));
        self.present.push_back(block_num);
    }

    /// Enforces configured limits
    fn enforce_limits(&mut self) {
        // enforce memory limits
        while self.present.len() >= self.in_memory_limit {
            // evict the oldest block in memory
            if let Some(block_num) = self.present.pop_front() {
                self.states.remove(&block_num);
            }
        }
    }
}

impl Default for HistoricalStates {
    fn default() -> Self {
        // enough in memory to store `DEFAULT_HISTORY_LIMIT` blocks in memory
        Self::new(DEFAULT_HISTORY_LIMIT)
    }
}

pub(super) type InMemoryStateDb = CacheStateDb<()>;
// pub(super) type InMemorySnapshot = StateSnapshot<()>;

impl Default for InMemoryStateDb {
    fn default() -> Self {
        CacheStateDb {
            db: (),
            storage: Default::default(),
            contract_state: Default::default(),
            shared_contract_classes: Arc::new(SharedContractClasses {
                classes: Default::default(),
                compiled_classes: Default::default(),
            }),
            compiled_class_hashes: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct EmptyStateProvider;

impl StateProvider for EmptyStateProvider {
    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        let _ = address;
        Ok(None)
    }

    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let _ = address;
        Ok(None)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let _ = address;
        let _ = storage_key;
        Ok(None)
    }
}

impl ContractClassProvider for EmptyStateProvider {
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<ContractClass>> {
        let _ = hash;
        Ok(None)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        let _ = hash;
        Ok(None)
    }
}

impl StateProofProvider for EmptyStateProvider {
    fn class_multiproof(&self, classes: Vec<ClassHash>) -> ProviderResult<MultiProof> {
        let _ = classes;
        Ok(MultiProof(Default::default()))
    }

    fn contract_multiproof(&self, addresses: Vec<ContractAddress>) -> ProviderResult<MultiProof> {
        let _ = addresses;
        Ok(MultiProof(Default::default()))
    }

    fn storage_multiproof(
        &self,
        address: ContractAddress,
        key: Vec<StorageKey>,
    ) -> ProviderResult<MultiProof> {
        let _ = address;
        let _ = key;
        Ok(MultiProof(Default::default()))
    }
}

impl StateRootProvider for EmptyStateProvider {
    fn classes_root(&self) -> ProviderResult<Felt> {
        Ok(Felt::ZERO)
    }

    fn contracts_root(&self) -> ProviderResult<Felt> {
        Ok(Felt::ZERO)
    }

    fn state_root(&self) -> ProviderResult<Felt> {
        Ok(Felt::ZERO)
    }

    fn storage_root(&self, contract: ContractAddress) -> ProviderResult<Option<Felt>> {
        let _ = contract;
        Ok(None)
    }
}
