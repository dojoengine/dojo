use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::da::L1DataAvailabilityMode;
use crate::version::ProtocolVersion;
use crate::{ContractAddress, Felt};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    pub parent_hash: BlockHash,
    pub number: BlockNumber,
    pub state_diff_length: u32,
    pub state_diff_commitment: Felt,
    pub transactions_commitment: Felt,
    pub receipts_commitment: Felt,
    pub events_commitment: Felt,
    pub state_root: Felt,
    pub timestamp: u64,
    pub sequencer_address: ContractAddress,
    pub l1_gas_prices: GasPrices,
    pub l1_data_gas_prices: GasPrices,
    pub l1_da_mode: L1DataAvailabilityMode,
    pub protocol_version: ProtocolVersion,
}
