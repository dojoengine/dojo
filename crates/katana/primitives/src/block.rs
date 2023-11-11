use serde::{Deserialize, Serialize};

use crate::contract::ContractAddress;
use crate::FieldElement;

/// Block state update type.
pub type StateUpdate = starknet::core::types::StateUpdate;

/// Block number type.
pub type BlockNumber = u64;
/// Block hash type.
pub type BlockHash = FieldElement;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub parent_hash: BlockHash,
    pub number: BlockNumber,
    pub gas_price: u128,
    pub timestamp: u64,
    pub state_root: FieldElement,
    pub sequencer_address: ContractAddress,
}
