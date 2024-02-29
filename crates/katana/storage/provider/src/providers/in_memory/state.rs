use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, FlattenedSierraClass,
    GenericContractInfo, Nonce, StorageKey, StorageValue,
};

use super::cache::{CacheSnapshotWithoutClasses, CacheStateDb, SharedContractClasses};
use crate::traits::contract::{ContractClassProvider, ContractInfoProvider};
use crate::traits::state::StateProvider;
use crate::ProviderResult;

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
pub(super) type InMemorySnapshot = StateSnapshot<()>;

impl Default for InMemoryStateDb {
    fn default() -> Self {
        CacheStateDb {
            db: (),
            storage: Default::default(),
            contract_state: Default::default(),
            shared_contract_classes: Arc::new(SharedContractClasses {
                sierra_classes: Default::default(),
                compiled_classes: Default::default(),
            }),
            compiled_class_hashes: Default::default(),
        }
    }
}

impl InMemoryStateDb {
    pub(crate) fn create_snapshot(&self) -> StateSnapshot<()> {
        StateSnapshot {
            inner: self.create_snapshot_without_classes(),
            classes: Arc::clone(&self.shared_contract_classes),
        }
    }
}

impl ContractInfoProvider for InMemorySnapshot {
    fn contract(&self, address: ContractAddress) -> ProviderResult<Option<GenericContractInfo>> {
        let info = self.inner.contract_state.get(&address).cloned();
        Ok(info)
    }
}

impl StateProvider for InMemorySnapshot {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let nonce = ContractInfoProvider::contract(&self, address)?.map(|i| i.nonce);
        Ok(nonce)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let value = self.inner.storage.get(&address).and_then(|s| s.get(&storage_key)).copied();
        Ok(value)
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        let class_hash = ContractInfoProvider::contract(&self, address)?.map(|i| i.class_hash);
        Ok(class_hash)
    }
}

impl ContractClassProvider for InMemorySnapshot {
    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            Ok(self.classes.sierra_classes.read().get(&hash).cloned())
        } else {
            Ok(None)
        }
    }

    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            Ok(self.classes.compiled_classes.read().get(&hash).cloned())
        } else {
            Ok(None)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        let hash = self.inner.compiled_class_hashes.get(&hash).cloned();
        Ok(hash)
    }
}

pub(super) struct LatestStateProvider(pub(super) Arc<InMemoryStateDb>);

impl ContractInfoProvider for LatestStateProvider {
    fn contract(&self, address: ContractAddress) -> ProviderResult<Option<GenericContractInfo>> {
        let info = self.0.contract_state.read().get(&address).cloned();
        Ok(info)
    }
}

impl StateProvider for LatestStateProvider {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let nonce = ContractInfoProvider::contract(&self, address)?.map(|i| i.nonce);
        Ok(nonce)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let value = self.0.storage.read().get(&address).and_then(|s| s.get(&storage_key)).copied();
        Ok(value)
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        let class_hash = ContractInfoProvider::contract(&self, address)?.map(|i| i.class_hash);
        Ok(class_hash)
    }
}

impl ContractClassProvider for LatestStateProvider {
    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        let class = self.0.shared_contract_classes.sierra_classes.read().get(&hash).cloned();
        Ok(class)
    }

    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        let class = self.0.shared_contract_classes.compiled_classes.read().get(&hash).cloned();
        Ok(class)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        let hash = self.0.compiled_class_hashes.read().get(&hash).cloned();
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::block::BlockHashOrNumber;
    use katana_primitives::contract::{GenericContractInfo, StorageKey};
    use starknet::macros::felt;

    use super::*;
    use crate::providers::in_memory::InMemoryProvider;
    use crate::traits::state::StateFactoryProvider;

    const STORAGE_KEY: StorageKey = felt!("0x1");

    const ADDR_1: ContractAddress = ContractAddress(felt!("0xADD1"));
    const ADDR_1_STORAGE_VALUE_AT_1: StorageKey = felt!("0x8080");
    const ADDR_1_STORAGE_VALUE_AT_2: StorageKey = felt!("0x1212");
    const ADDR_1_STORAGE_VALUE_AT_3: StorageKey = felt!("0x3434");
    const ADDR_1_CLASS_HASH_AT_1: ClassHash = felt!("0x4337");
    const ADDR_1_CLASS_HASH_AT_2: ClassHash = felt!("0x7334");
    const ADDR_1_NONCE_AT_1: Nonce = felt!("0x1");
    const ADDR_1_NONCE_AT_2: Nonce = felt!("0x2");
    const ADDR_1_NONCE_AT_3: Nonce = felt!("0x3");

    const ADDR_2: ContractAddress = ContractAddress(felt!("0xADD2"));
    const ADDR_2_STORAGE_VALUE_AT_1: StorageKey = felt!("0x9090");
    const ADDR_2_STORAGE_VALUE_AT_2: StorageKey = felt!("1313");
    const ADDR_2_STORAGE_VALUE_AT_3: StorageKey = felt!("5555");
    const ADDR_2_CLASS_HASH_AT_1: ClassHash = felt!("0x1559");
    const ADDR_2_CLASS_HASH_AT_2: ClassHash = felt!("0x9551");
    const ADDR_2_NONCE_AT_1: Nonce = felt!("0x1");
    const ADDR_2_NONCE_AT_2: Nonce = felt!("0x2");
    const ADDR_2_NONCE_AT_3: Nonce = felt!("0x3");

    fn create_mock_state() -> InMemoryStateDb {
        let storage = HashMap::from([
            (ADDR_1, HashMap::from([(STORAGE_KEY, ADDR_1_STORAGE_VALUE_AT_1)])),
            (ADDR_2, HashMap::from([(STORAGE_KEY, ADDR_2_STORAGE_VALUE_AT_1)])),
        ]);

        let contract_state = HashMap::from([
            (
                ADDR_1,
                GenericContractInfo { nonce: felt!("0x1"), class_hash: ADDR_1_CLASS_HASH_AT_1 },
            ),
            (
                ADDR_2,
                GenericContractInfo { nonce: felt!("0x1"), class_hash: ADDR_2_CLASS_HASH_AT_1 },
            ),
        ]);

        InMemoryStateDb {
            storage: storage.into(),
            contract_state: contract_state.into(),
            ..Default::default()
        }
    }

    #[test]
    fn latest_state_provider() {
        let state = create_mock_state();

        let mut provider = InMemoryProvider::new();
        provider.state = Arc::new(state);

        let latest_state_provider = StateFactoryProvider::latest(&provider).unwrap();

        assert_eq!(latest_state_provider.nonce(ADDR_1).unwrap(), Some(felt!("0x1")));
        assert_eq!(
            latest_state_provider.storage(ADDR_1, STORAGE_KEY).unwrap(),
            Some(ADDR_1_STORAGE_VALUE_AT_1)
        );
        assert_eq!(
            latest_state_provider.class_hash_of_contract(ADDR_1).unwrap(),
            Some(felt!("0x4337"))
        );

        assert_eq!(latest_state_provider.nonce(ADDR_2).unwrap(), Some(felt!("0x1")));
        assert_eq!(
            latest_state_provider.storage(ADDR_2, STORAGE_KEY).unwrap(),
            Some(ADDR_2_STORAGE_VALUE_AT_1)
        );
        assert_eq!(
            latest_state_provider.class_hash_of_contract(ADDR_2).unwrap(),
            Some(felt!("0x1559"))
        );
    }

    #[test]
    fn historical_state_provider() {
        // setup

        let state = create_mock_state();
        // create snapshot 1
        let snapshot_1 = state.create_snapshot();

        state.storage.write().extend([
            (ADDR_1, HashMap::from([(STORAGE_KEY, ADDR_1_STORAGE_VALUE_AT_2)])),
            (ADDR_2, HashMap::from([(STORAGE_KEY, ADDR_2_STORAGE_VALUE_AT_2)])),
        ]);

        state.contract_state.write().extend([
            (
                ADDR_1,
                GenericContractInfo {
                    nonce: ADDR_1_NONCE_AT_2,
                    class_hash: ADDR_1_CLASS_HASH_AT_2,
                },
            ),
            (
                ADDR_2,
                GenericContractInfo {
                    nonce: ADDR_2_NONCE_AT_2,
                    class_hash: ADDR_2_CLASS_HASH_AT_2,
                },
            ),
        ]);

        // create snapshot 2
        let snapshot_2 = state.create_snapshot();

        state.storage.write().extend([
            (ADDR_1, HashMap::from([(STORAGE_KEY, ADDR_1_STORAGE_VALUE_AT_3)])),
            (ADDR_2, HashMap::from([(STORAGE_KEY, ADDR_2_STORAGE_VALUE_AT_3)])),
        ]);

        state.contract_state.write().entry(ADDR_1).and_modify(|e| e.nonce = ADDR_1_NONCE_AT_3);
        state.contract_state.write().entry(ADDR_2).and_modify(|e| e.nonce = ADDR_2_NONCE_AT_3);

        let mut provider = InMemoryProvider::new();
        provider.state = Arc::new(state);
        provider.historical_states.write().insert(1, Box::new(snapshot_1));
        provider.historical_states.write().insert(2, Box::new(snapshot_2));

        // check latest state

        let latest_state_provider = StateFactoryProvider::latest(&provider).unwrap();

        assert_eq!(
            latest_state_provider.nonce(ADDR_1).unwrap(),
            Some(ADDR_1_NONCE_AT_3),
            "nonce must be updated"
        );
        assert_eq!(
            latest_state_provider.storage(ADDR_1, STORAGE_KEY).unwrap(),
            Some(ADDR_1_STORAGE_VALUE_AT_3),
            "storage must be updated"
        );
        assert_eq!(
            latest_state_provider.class_hash_of_contract(ADDR_1).unwrap(),
            Some(ADDR_1_CLASS_HASH_AT_2)
        );

        assert_eq!(
            latest_state_provider.nonce(ADDR_2).unwrap(),
            Some(ADDR_2_NONCE_AT_3),
            "nonce must be updated"
        );
        assert_eq!(
            latest_state_provider.storage(ADDR_2, STORAGE_KEY).unwrap(),
            Some(ADDR_2_STORAGE_VALUE_AT_3),
            "storage must be updated"
        );
        assert_eq!(
            latest_state_provider.class_hash_of_contract(ADDR_2).unwrap(),
            Some(ADDR_2_CLASS_HASH_AT_2)
        );

        // check historical state at 1

        let historical_state_provider =
            StateFactoryProvider::historical(&provider, BlockHashOrNumber::Num(1))
                .unwrap()
                .unwrap();

        assert_eq!(historical_state_provider.nonce(ADDR_1).unwrap(), Some(ADDR_1_NONCE_AT_1));
        assert_eq!(
            historical_state_provider.storage(ADDR_1, STORAGE_KEY).unwrap(),
            Some(ADDR_1_STORAGE_VALUE_AT_1)
        );
        assert_eq!(
            historical_state_provider.class_hash_of_contract(ADDR_1).unwrap(),
            Some(ADDR_1_CLASS_HASH_AT_1)
        );

        assert_eq!(historical_state_provider.nonce(ADDR_2).unwrap(), Some(ADDR_2_NONCE_AT_1));
        assert_eq!(
            historical_state_provider.storage(ADDR_2, STORAGE_KEY).unwrap(),
            Some(ADDR_2_STORAGE_VALUE_AT_1)
        );
        assert_eq!(
            historical_state_provider.class_hash_of_contract(ADDR_2).unwrap(),
            Some(ADDR_2_CLASS_HASH_AT_1)
        );

        // check historical state at 2

        let historical_state_provider =
            StateFactoryProvider::historical(&provider, BlockHashOrNumber::Num(2))
                .unwrap()
                .unwrap();

        assert_eq!(historical_state_provider.nonce(ADDR_1).unwrap(), Some(ADDR_1_NONCE_AT_2));
        assert_eq!(
            historical_state_provider.storage(ADDR_1, STORAGE_KEY).unwrap(),
            Some(ADDR_1_STORAGE_VALUE_AT_2)
        );
        assert_eq!(
            historical_state_provider.class_hash_of_contract(ADDR_1).unwrap(),
            Some(ADDR_1_CLASS_HASH_AT_2)
        );

        assert_eq!(historical_state_provider.nonce(ADDR_2).unwrap(), Some(ADDR_2_NONCE_AT_2));
        assert_eq!(
            historical_state_provider.storage(ADDR_2, STORAGE_KEY).unwrap(),
            Some(ADDR_2_STORAGE_VALUE_AT_2)
        );
        assert_eq!(
            historical_state_provider.class_hash_of_contract(ADDR_2).unwrap(),
            Some(ADDR_2_CLASS_HASH_AT_2)
        );
    }
}
