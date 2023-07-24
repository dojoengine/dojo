use std::collections::HashMap;

use blockifier::block_context::BlockContext;
use starknet::core::types::{BlockId, BlockTag, FieldElement};

use self::{block::Block, transaction::KnownTransaction};
use crate::backend::storage::block::PartialHeader;

pub mod block;
pub mod transaction;

// TODO: can we wrap all the fields in a `RwLock` to prevent read blocking?
#[derive(Debug, Default)]
pub struct BlockchainStorage {
    /// Mapping from block hash -> block
    pub blocks: HashMap<FieldElement, Block>,
    /// Mapping from block number -> block hash
    pub hashes: HashMap<u64, FieldElement>,
    /// The latest block hash
    pub latest_hash: FieldElement,
    /// The latest block number
    pub latest_number: u64,
    /// Mapping of all known transactions from its transaction hash
    pub transactions: HashMap<FieldElement, KnownTransaction>,
}

impl BlockchainStorage {
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
            transactions: HashMap::default(),
        }
    }

    pub fn total_blocks(&self) -> usize {
        self.blocks.len()
    }

    /// Returns the block hash based on the block id
    pub fn hash(&self, block: BlockId) -> Option<FieldElement> {
        match block {
            BlockId::Tag(BlockTag::Pending) => None,
            BlockId::Tag(BlockTag::Latest) => Some(self.latest_hash),
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(num) => self.hashes.get(&num).copied(),
        }
    }
}
