use starknet::core::crypto::compute_hash_on_elements;

use crate::contract::ContractAddress;
use crate::da::L1DataAvailabilityMode;
use crate::transaction::{ExecutableTxWithHash, TxHash, TxWithHash};
use crate::version::ProtocolVersion;
use crate::Felt;

pub type BlockIdOrTag = starknet::core::types::BlockId;
pub type BlockTag = starknet::core::types::BlockTag;

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BlockHashOrNumber {
    Hash(BlockHash),
    Num(BlockNumber),
}

impl std::fmt::Display for BlockHashOrNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockHashOrNumber::Num(num) => write!(f, "{num}"),
            BlockHashOrNumber::Hash(hash) => write!(f, "{hash:#x}"),
        }
    }
}

/// Block number type.
pub type BlockNumber = u64;
/// Block hash type.
pub type BlockHash = Felt;

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
    pub number: BlockNumber,
    pub parent_hash: Felt,
    pub timestamp: u64,
    pub sequencer_address: ContractAddress,
    pub version: ProtocolVersion,
    pub l1_gas_prices: GasPrices,
    pub l1_data_gas_prices: GasPrices,
    pub l1_da_mode: L1DataAvailabilityMode,
}

// TODO: change names to wei and fri
/// The L1 gas prices.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "UPPERCASE"))]
pub struct GasPrices {
    /// The price of one unit of the given resource, denominated in wei
    pub eth: u128,
    /// The price of one unit of the given resource, denominated in fri (the smallest unit of STRK,
    /// equivalent to 10^-18 STRK)
    pub strk: u128,
}

impl GasPrices {
    pub fn new(wei_gas_price: u128, fri_gas_price: u128) -> Self {
        Self { eth: wei_gas_price, strk: fri_gas_price }
    }
}

/// Represents a block header.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    pub parent_hash: BlockHash,
    pub number: BlockNumber,
    pub timestamp: u64,
    pub state_root: Felt,
    pub sequencer_address: ContractAddress,
    pub protocol_version: ProtocolVersion,
    pub l1_gas_prices: GasPrices,
    pub l1_data_gas_prices: GasPrices,
    pub l1_da_mode: L1DataAvailabilityMode,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            timestamp: 0,
            number: BlockNumber::default(),
            state_root: Felt::default(),
            parent_hash: BlockHash::default(),
            l1_gas_prices: GasPrices::default(),
            protocol_version: ProtocolVersion::default(),
            sequencer_address: ContractAddress::default(),
            l1_data_gas_prices: GasPrices::default(),
            l1_da_mode: L1DataAvailabilityMode::Calldata,
        }
    }
}

impl Header {
    pub fn new(partial_header: PartialHeader, state_root: Felt) -> Self {
        Self {
            state_root,
            number: partial_header.number,
            protocol_version: partial_header.version,
            timestamp: partial_header.timestamp,
            parent_hash: partial_header.parent_hash,
            sequencer_address: partial_header.sequencer_address,
            l1_gas_prices: partial_header.l1_gas_prices,
            l1_da_mode: partial_header.l1_da_mode,
            l1_data_gas_prices: partial_header.l1_data_gas_prices,
        }
    }

    /// Computes the hash of the header.
    pub fn compute_hash(&self) -> Felt {
        compute_hash_on_elements(&vec![
            self.number.into(),            // block number
            self.state_root,               // state root
            self.sequencer_address.into(), // sequencer address
            self.timestamp.into(),         // block timestamp
            Felt::ZERO,                    // transaction commitment
            Felt::ZERO,                    // event commitment
            Felt::ZERO,                    // protocol version
            Felt::ZERO,                    // extra data
            self.parent_hash,              // parent hash
        ])
    }

    fn seal(self) -> SealedHeader {
        let hash = self.compute_hash();
        SealedHeader { hash, header: self }
    }
}

/// Represents a Starknet full block.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
///
/// Block whose commitment has been computed.
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

impl From<BlockHashOrNumber> for BlockIdOrTag {
    fn from(value: BlockHashOrNumber) -> Self {
        match value {
            BlockHashOrNumber::Hash(hash) => BlockIdOrTag::Hash(hash),
            BlockHashOrNumber::Num(number) => BlockIdOrTag::Number(number),
        }
    }
}

/// A block that can executed. This is a block whose transactions includes
/// all the necessary information to be executed.
#[derive(Debug, Clone)]
pub struct ExecutableBlock {
    pub header: PartialHeader,
    pub body: Vec<ExecutableTxWithHash>,
}
