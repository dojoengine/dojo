use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use katana_primitives::block::BlockNumber;

use super::cache::{CacheSnapshotWithoutClasses, CacheStateDb, SharedContractClasses};
use crate::traits::state::StateProvider;

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
