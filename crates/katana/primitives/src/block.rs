use starknet::core::crypto::compute_hash_on_elements;

use crate::contract::ContractAddress;
use crate::transaction::{TxHash, TxWithHash};
use crate::FieldElement;

pub type BlockIdOrTag = starknet::core::types::BlockId;
pub type BlockTag = starknet::core::types::BlockTag;

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

/// Finality status of a canonical block.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FinalityStatus {
    AcceptedOnL2,
    AcceptedOnL1,
}

/// Represents a partial block header.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartialHeader {
    pub parent_hash: FieldElement,
    pub number: BlockNumber,
    pub gas_price: u128,
    pub timestamp: u64,
    pub sequencer_address: ContractAddress,
}

/// Represents a block header.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    pub parent_hash: BlockHash,
    pub number: BlockNumber,
    pub gas_price: u128,
    pub timestamp: u64,
    pub state_root: FieldElement,
    pub sequencer_address: ContractAddress,
}

impl Header {
    pub fn new(partial_header: PartialHeader, state_root: FieldElement) -> Self {
        Self {
            state_root,
            number: partial_header.number,
            gas_price: partial_header.gas_price,
            timestamp: partial_header.timestamp,
            parent_hash: partial_header.parent_hash,
            sequencer_address: partial_header.sequencer_address,
        }
    }

    /// Computes the hash of the header.
    pub fn compute_hash(&self) -> FieldElement {
        compute_hash_on_elements(&vec![
            self.number.into(),            // block number
            self.state_root,               // state root
            self.sequencer_address.into(), // sequencer address
            self.timestamp.into(),         // block timestamp
            FieldElement::ZERO,            // transaction commitment
            FieldElement::ZERO,            // event commitment
            FieldElement::ZERO,            // protocol version
            FieldElement::ZERO,            // extra data
            self.parent_hash,              // parent hash
        ])
    }

    fn seal(self) -> SealedHeader {
        let hash = self.compute_hash();
        SealedHeader { hash, header: self }
    }
}

/// Represents a Starknet full block.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Block {
    pub header: Header,
    pub body: Vec<TxWithHash>,
}

/// A block with only the transaction hashes.
#[derive(Debug, Clone)]
pub struct BlockWithTxHashes {
    pub header: Header,
    pub body: Vec<TxHash>,
}

impl Block {
    pub fn seal(self) -> SealedBlock {
        SealedBlock { header: self.header.seal(), body: self.body }
    }
}

#[derive(Debug, Clone)]
pub struct SealedHeader {
    /// The hash of the header.
    pub hash: BlockHash,
    /// The block header.
    pub header: Header,
}

/// A full Starknet block that has been sealed.
#[derive(Debug, Clone)]
pub struct SealedBlock {
    /// The sealed block header.
    pub header: SealedHeader,
    /// The block body.
    pub body: Vec<TxWithHash>,
}

/// A sealed block along with its status.
pub struct SealedBlockWithStatus {
    pub block: SealedBlock,
    /// The block status.
    pub status: FinalityStatus,
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
