use starknet::core::crypto::compute_hash_on_elements;

use crate::contract::ContractAddress;
use crate::transaction::{ExecutableTxWithHash, TxHash, TxWithHash};
use crate::version::Version;
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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    pub gas_prices: GasPrices,
    pub timestamp: u64,
    pub sequencer_address: ContractAddress,
    pub version: Version,
}

/// The L1 gas prices.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "UPPERCASE"))]
pub struct GasPrices {
    /// The price of one unit of the given resource, denominated in wei
    pub eth: u64,
    /// The price of one unit of the given resource, denominated in strk
    pub strk: u64,
}

impl GasPrices {
    pub fn new(eth_gas_price: u64, strk_gas_price: u64) -> Self {
        Self { eth: eth_gas_price, strk: strk_gas_price }
    }
}

/// Represents a block header.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    pub parent_hash: BlockHash,
    pub number: BlockNumber,
    pub gas_prices: GasPrices,
    pub timestamp: u64,
    pub state_root: FieldElement,
    pub sequencer_address: ContractAddress,
    pub version: Version,
}

impl Header {
    pub fn new(
        partial_header: PartialHeader,
        number: BlockNumber,
        state_root: FieldElement,
    ) -> Self {
        Self {
            number,
            state_root,
            version: partial_header.version,
            timestamp: partial_header.timestamp,
            gas_prices: partial_header.gas_prices,
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Block {
    pub header: Header,
    pub body: Vec<TxWithHash>,
}

/// A block with only the transaction hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockWithTxHashes {
    pub header: Header,
    pub body: Vec<TxHash>,
}

impl Block {
    /// Seals the block. This computes the hash of the block.
    pub fn seal(self) -> SealedBlock {
        SealedBlock { header: self.header.seal(), body: self.body }
    }

    /// Seals the block with a given hash.
    pub fn seal_with_hash(self, hash: BlockHash) -> SealedBlock {
        SealedBlock { header: SealedHeader { hash, header: self.header }, body: self.body }
    }

    /// Seals the block with a given block hash and status.
    pub fn seal_with_hash_and_status(
        self,
        hash: BlockHash,
        status: FinalityStatus,
    ) -> SealedBlockWithStatus {
        SealedBlockWithStatus { block: self.seal_with_hash(hash), status }
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

impl SealedBlock {
    /// Unseal the block.
    pub fn unseal(self) -> Block {
        Block { header: self.header.header, body: self.body }
    }
}

/// A sealed block along with its status.
#[derive(Debug, Clone)]
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

/// A block that can executed. This is a block whose transactions includes
/// all the necessary information to be executed.
pub struct ExecutableBlock {
    pub header: PartialHeader,
    pub body: Vec<ExecutableTxWithHash>,
}
