use std::collections::HashMap;

use katana_primitives::block::{Block, StateUpdate};
use katana_primitives::transaction::Transaction;
use katana_primitives::FieldElement;

#[derive(Debug, Default)]
pub struct InMemoryProvider {
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
    pub transactions: HashMap<FieldElement, Transaction>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        Self::default()
    }
}
