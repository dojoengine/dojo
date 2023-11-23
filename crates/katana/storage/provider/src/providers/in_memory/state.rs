use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use anyhow::Result;
use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, GenericContractInfo,
    Nonce, SierraClass, StorageKey, StorageValue,
};
use parking_lot::RwLock;

use crate::traits::state::{StateProvider, StateProviderExt};

type ContractStorageMap = HashMap<(ContractAddress, StorageKey), StorageValue>;
type ContractStateMap = HashMap<ContractAddress, GenericContractInfo>;

type SierraClassesMap = HashMap<ClassHash, SierraClass>;
type CompiledClassesMap = HashMap<ClassHash, CompiledContractClass>;
type CompiledClassHashesMap = HashMap<ClassHash, CompiledClassHash>;

pub struct StateSnapshot {
    pub contract_state: ContractStateMap,
    pub storage: ContractStorageMap,
    pub compiled_class_hashes: CompiledClassHashesMap,
    pub shared_sierra_classes: Arc<RwLock<SierraClassesMap>>,
    pub shared_compiled_classes: Arc<RwLock<CompiledClassesMap>>,
}

#[derive(Default)]
pub struct InMemoryState {
    pub contract_state: RwLock<ContractStateMap>,
    pub storage: RwLock<ContractStorageMap>,
    pub compiled_class_hashes: RwLock<CompiledClassHashesMap>,
    pub shared_sierra_classes: Arc<RwLock<SierraClassesMap>>,
    pub shared_compiled_classes: Arc<RwLock<CompiledClassesMap>>,
}

impl InMemoryState {
    pub(crate) fn create_snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            storage: self.storage.read().clone(),
            contract_state: self.contract_state.read().clone(),
            compiled_class_hashes: self.compiled_class_hashes.read().clone(),
            shared_sierra_classes: self.shared_sierra_classes.clone(),
            shared_compiled_classes: self.shared_compiled_classes.clone(),
        }
    }
}

const DEFAULT_HISTORY_LIMIT: usize = 500;
const MIN_HISTORY_LIMIT: usize = 10;

/// Represents the complete state of a single block.
///
/// It should store at N - 1 states, where N is the latest block number.
pub struct HistoricalStates {
    /// The states at a certain block based on the block number
    states: HashMap<BlockNumber, Arc<StateSnapshot>>,
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
    pub fn get(&self, block_num: &BlockNumber) -> Option<&Arc<StateSnapshot>> {
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
    pub fn insert(&mut self, block_num: BlockNumber, state: StateSnapshot) {
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

pub struct LatestStateProvider(pub(super) Arc<InMemoryState>);

impl StateProvider for LatestStateProvider {
    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
        let nonce = self.0.contract_state.read().get(&address).map(|info| info.nonce);
        Ok(nonce)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        let value = self.0.storage.read().get(&(address, storage_key)).cloned();
        Ok(value)
    }

    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        let class = self.0.shared_compiled_classes.read().get(&hash).cloned();
        Ok(class)
    }

    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
        let class_hash = self.0.contract_state.read().get(&address).map(|info| info.class_hash);
        Ok(class_hash)
    }
}

impl StateProviderExt for LatestStateProvider {
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<SierraClass>> {
        let class = self.0.shared_sierra_classes.read().get(&hash).cloned();
        Ok(class)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>> {
        let hash = self.0.compiled_class_hashes.read().get(&hash).cloned();
        Ok(hash)
    }
}

pub struct SnapshotStateProvider(pub(super) Arc<StateSnapshot>);

impl StateProvider for SnapshotStateProvider {
    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
        let nonce = self.0.contract_state.get(&address).map(|info| info.nonce);
        Ok(nonce)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        let value = self.0.storage.get(&(address, storage_key)).cloned();
        Ok(value)
    }

    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        let class = self.0.shared_compiled_classes.read().get(&hash).cloned();
        Ok(class)
    }

    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
        let class_hash = self.0.contract_state.get(&address).map(|info| info.class_hash);
        Ok(class_hash)
    }
}

impl StateProviderExt for SnapshotStateProvider {
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<SierraClass>> {
        let class = self.0.shared_sierra_classes.read().get(&hash).cloned();
        Ok(class)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>> {
        let hash = self.0.compiled_class_hashes.get(&hash).cloned();
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::block::BlockHashOrNumber;
    use katana_primitives::contract::StorageKey;
    use starknet::macros::felt;

    use super::*;
    use crate::providers::in_memory::tests::create_mock_provider;
    use crate::traits::state::StateFactoryProvider;

    const STORAGE_KEY: StorageKey = felt!("0x1");

    const ADDR_1: ContractAddress = ContractAddress(felt!("0xADD1"));
    const ADDR_1_STORAGE_VALUE_AT_1: StorageKey = felt!("0x8080");
    const ADDR_1_STORAGE_VALUE_AT_2: StorageKey = felt!("0x1212");
    const ADDR_1_CLASS_HASH_AT_1: ClassHash = felt!("0x4337");
    const ADDR_1_CLASS_HASH_AT_2: ClassHash = felt!("0x7334");
    const ADDR_1_NONCE_AT_1: Nonce = felt!("0x1");
    const ADDR_1_NONCE_AT_2: Nonce = felt!("0x2");

    const ADDR_2: ContractAddress = ContractAddress(felt!("0xADD2"));
    const ADDR_2_STORAGE_VALUE_AT_1: StorageKey = felt!("0x9090");
    const ADDR_2_STORAGE_VALUE_AT_2: StorageKey = felt!("1313");
    const ADDR_2_CLASS_HASH_AT_1: ClassHash = felt!("0x1559");
    const ADDR_2_CLASS_HASH_AT_2: ClassHash = felt!("0x9551");
    const ADDR_2_NONCE_AT_1: Nonce = felt!("0x1");
    const ADDR_2_NONCE_AT_2: Nonce = felt!("0x2");

    fn create_mock_state() -> InMemoryState {
        let storage = HashMap::from([
            ((ADDR_1, STORAGE_KEY), ADDR_1_STORAGE_VALUE_AT_1),
            ((ADDR_2, STORAGE_KEY), ADDR_2_STORAGE_VALUE_AT_1),
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

        InMemoryState {
            storage: storage.into(),
            contract_state: contract_state.into(),
            ..Default::default()
        }
    }

    #[test]
    fn latest_state_provider() {
        let state = create_mock_state();

        let mut provider = create_mock_provider();
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
        let snapshot = state.create_snapshot();

        state.storage.write().extend([
            ((ADDR_1, STORAGE_KEY), ADDR_1_STORAGE_VALUE_AT_2),
            ((ADDR_2, STORAGE_KEY), ADDR_2_STORAGE_VALUE_AT_2),
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

        let mut provider = create_mock_provider();
        provider.state = Arc::new(state);
        provider.historical_states.write().insert(1, snapshot);

        // check latest state

        let latest_state_provider = StateFactoryProvider::latest(&provider).unwrap();

        assert_eq!(latest_state_provider.nonce(ADDR_1).unwrap(), Some(ADDR_1_NONCE_AT_2));
        assert_eq!(
            latest_state_provider.storage(ADDR_1, STORAGE_KEY).unwrap(),
            Some(ADDR_1_STORAGE_VALUE_AT_2)
        );
        assert_eq!(
            latest_state_provider.class_hash_of_contract(ADDR_1).unwrap(),
            Some(ADDR_1_CLASS_HASH_AT_2)
        );

        assert_eq!(latest_state_provider.nonce(ADDR_2).unwrap(), Some(ADDR_2_NONCE_AT_2));
        assert_eq!(
            latest_state_provider.storage(ADDR_2, STORAGE_KEY).unwrap(),
            Some(ADDR_2_STORAGE_VALUE_AT_2)
        );
        assert_eq!(
            latest_state_provider.class_hash_of_contract(ADDR_2).unwrap(),
            Some(ADDR_2_CLASS_HASH_AT_2)
        );

        // check historical state

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
    }
}
