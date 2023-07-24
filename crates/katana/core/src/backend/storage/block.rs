use starknet::core::types::{BlockStatus, FieldElement};

use super::transaction::{IncludedTransaction, TransactionOutput};

#[derive(Debug, Clone)]
pub struct Header {
    pub parent_hash: FieldElement,
    pub number: u64,
    pub gas_price: u128,
    pub timestamp: u128,
    pub state_root: FieldElement,
    pub sequencer_address: FieldElement,
}

impl Header {
    pub fn hash(&self) -> FieldElement {
        unimplemented!("compute the block hash from its header")
    }
}

#[derive(Debug)]
pub struct Block {
    pub header: Header,
    pub status: BlockStatus,
    pub transactions: Vec<IncludedTransaction>,
    pub outputs: Vec<TransactionOutput>,
}
