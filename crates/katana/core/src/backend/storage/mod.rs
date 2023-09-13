use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use blockifier::block_context::BlockContext;
use parking_lot::RwLock;
use starknet::core::types::{BlockId, BlockTag, FieldElement, StateDiff, StateUpdate};

use self::block::Block;
use self::transaction::KnownTransaction;
use crate::backend::storage::block::PartialHeader;
use crate::db::StateRefDb;

pub mod block;
pub mod transaction;

const DEFAULT_HISTORY_LIMIT: usize = 500;
const MIN_HISTORY_LIMIT: usize = 10;

/// Represents the complete state of a single block
pub struct InMemoryBlockStates {
    /// The states at a certain block
    states: HashMap<FieldElement, StateRefDb>,
    /// How many states to store at most
    in_memory_limit: usize,
    /// minimum amount of states we keep in memory
    min_in_memory_limit: usize,
    /// all states present, used to enforce `in_memory_limit`
    present: VecDeque<FieldElement>,
}

impl InMemoryBlockStates {
    pub fn new(limit: usize) -> Self {
        Self {
            states: Default::default(),
            in_memory_limit: limit,
            min_in_memory_limit: limit.min(MIN_HISTORY_LIMIT),
            present: Default::default(),
        }
    }

    /// Returns the state for the given `hash` if present
    pub fn get(&self, hash: &FieldElement) -> Option<&StateRefDb> {
        self.states.get(hash)
    }

    /// Inserts a new (hash -> state) pair
    ///
    /// When the configured limit for the number of states that can be stored in memory is reached,
    /// the oldest state is removed.
    ///
    /// Since we keep a snapshot of the entire state as history, the size of the state will increase
    /// with the transactions processed. To counter this, we gradually decrease the cache limit with
    /// the number of states/blocks until we reached the `min_limit`.
    pub fn insert(&mut self, hash: FieldElement, state: StateRefDb) {
        if self.present.len() >= self.in_memory_limit {
            // once we hit the max limit we gradually decrease it
            self.in_memory_limit =
                self.in_memory_limit.saturating_sub(1).max(self.min_in_memory_limit);
        }

        self.enforce_limits();
        self.states.insert(hash, state);
        self.present.push_back(hash);
    }

    /// Enforces configured limits
    fn enforce_limits(&mut self) {
        // enforce memory limits
        while self.present.len() >= self.in_memory_limit {
            // evict the oldest block in memory
            if let Some(hash) = self.present.pop_front() {
                self.states.remove(&hash);
            }
        }
    }
}

impl Default for InMemoryBlockStates {
    fn default() -> Self {
        // enough in memory to store `DEFAULT_HISTORY_LIMIT` blocks in memory
        Self::new(DEFAULT_HISTORY_LIMIT)
    }
}

#[derive(Debug, Default)]
pub struct Storage {
    /// Mapping from block hash -> block
    pub blocks: HashMap<FieldElement, Block>,
    /// Mapping from block number -> block hash
    pub hashes: HashMap<u64, FieldElement>,
    /// Mapping from block number -> state update
    pub state_update: HashMap<FieldElement, StateUpdate>,
    /// The latest block hash
    pub latest_hash: FieldElement,
    /// The latest block number
    pub latest_number: u64,
    /// Mapping of all known transactions from its transaction hash
    pub transactions: HashMap<FieldElement, KnownTransaction>,
}

impl Storage {
    /// Creates a new blockchain from a genesis block
    pub fn new(block_context: &BlockContext) -> Self {
        let partial_header = PartialHeader {
            parent_hash: FieldElement::ZERO,
            gas_price: block_context.gas_price,
            number: block_context.block_number.0,
            timestamp: block_context.block_timestamp.0,
            sequencer_address: (*block_context.sequencer_address.0.key()).into(),
        };

        // Create a dummy genesis block
        let genesis_block = Block::new(partial_header, vec![], vec![]);
        let genesis_hash = genesis_block.header.hash();
        let genesis_number = 0u64;

        Self {
            blocks: HashMap::from([(genesis_hash, genesis_block)]),
            hashes: HashMap::from([(genesis_number, genesis_hash)]),
            latest_hash: genesis_hash,
            latest_number: genesis_number,
            state_update: HashMap::default(),
            transactions: HashMap::default(),
        }
    }

    /// Creates a new blockchain from a forked network
    pub fn new_forked(latest_number: u64, latest_hash: FieldElement) -> Self {
        Self {
            latest_hash,
            latest_number,
            blocks: HashMap::default(),
            hashes: HashMap::from([(latest_number, latest_hash)]),
            state_update: HashMap::default(),
            transactions: HashMap::default(),
        }
    }

    pub fn block_by_number(&self, number: u64) -> Option<&Block> {
        self.hashes.get(&number).and_then(|hash| self.blocks.get(hash))
    }
}

pub struct Blockchain {
    pub storage: Arc<RwLock<Storage>>,
}

impl Blockchain {
    pub fn new(storage: Arc<RwLock<Storage>>) -> Self {
        Self { storage }
    }

    pub fn new_forked(latest_number: u64, latest_hash: FieldElement) -> Self {
        Self::new(Arc::new(RwLock::new(Storage::new_forked(latest_number, latest_hash))))
    }

    /// Returns the block hash based on the block id
    pub fn block_hash(&self, block: BlockId) -> Option<FieldElement> {
        match block {
            BlockId::Tag(BlockTag::Pending) => None,
            BlockId::Tag(BlockTag::Latest) => Some(self.storage.read().latest_hash),
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(num) => self.storage.read().hashes.get(&num).copied(),
        }
    }

    pub fn total_blocks(&self) -> usize {
        self.storage.read().blocks.len()
    }

    /// Appends a new block to the chain and store the state diff.
    pub fn append_block(&self, hash: FieldElement, block: Block, state_diff: StateDiff) {
        let number = block.header.number;
        let mut storage = self.storage.write();

        assert_eq!(storage.latest_number + 1, number);

        let old_root = storage
            .blocks
            .get(&storage.latest_hash)
            .map(|b| b.header.state_root)
            .unwrap_or_default();

        let state_update = StateUpdate {
            block_hash: hash,
            new_root: block.header.state_root,
            old_root,
            state_diff,
        };

        storage.latest_hash = hash;
        storage.latest_number = number;
        storage.blocks.insert(hash, block);
        storage.hashes.insert(number, hash);
        storage.state_update.insert(hash, state_update);
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::backend::in_memory_db::MemDb;

    #[test]
    fn remove_old_state_when_limit_is_reached() {
        let mut in_memory_state = InMemoryBlockStates::new(2);

        in_memory_state
            .insert(FieldElement::from_str("0x1").unwrap(), StateRefDb::new(MemDb::new()));
        in_memory_state
            .insert(FieldElement::from_str("0x2").unwrap(), StateRefDb::new(MemDb::new()));

        assert!(in_memory_state.states.get(&FieldElement::from_str("0x1").unwrap()).is_some());
        assert!(in_memory_state.states.get(&FieldElement::from_str("0x2").unwrap()).is_some());
        assert_eq!(in_memory_state.present.len(), 2);

        in_memory_state
            .insert(FieldElement::from_str("0x3").unwrap(), StateRefDb::new(MemDb::new()));

        assert_eq!(in_memory_state.present.len(), 2);
        assert!(in_memory_state.states.get(&FieldElement::from_str("0x1").unwrap()).is_none());
        assert!(in_memory_state.states.get(&FieldElement::from_str("0x2").unwrap()).is_some());
        assert!(in_memory_state.states.get(&FieldElement::from_str("0x3").unwrap()).is_some());
    }
}
