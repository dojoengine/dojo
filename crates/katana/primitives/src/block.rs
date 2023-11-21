use crate::contract::ContractAddress;
use crate::transaction::Transaction;
use crate::FieldElement;

/// Block state update type.
pub type StateUpdate = starknet::core::types::StateUpdate;

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BlockHashOrNumber {
    Hash(BlockHash),
    Num(BlockNumber),
}

/// Block number type.
pub type BlockNumber = u64;
/// Block hash type.
pub type BlockHash = FieldElement;

/// Represents a block header.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    pub parent_hash: BlockHash,
    pub number: BlockNumber,
    pub gas_price: u128,
    pub timestamp: u64,
    pub state_root: FieldElement,
    pub sequencer_address: ContractAddress,
}

/// Represents a Starknet full block.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Block {
    header: Header,
    body: Vec<Transaction>,
}

impl From<BlockNumber> for BlockHashOrNumber {
    fn from(number: BlockNumber) -> Self {
        Self::Num(number)
    }
}

impl From<BlockHash> for BlockHashOrNumber {
    fn from(hash: BlockHash) -> Self {
        Self::Hash(hash)
    }
}
