use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;

pub type StateUpdate = starknet::core::types::StateUpdate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub parent_hash: FieldElement,
    pub number: u64,
    pub gas_price: u128,
    pub timestamp: u64,
    pub state_root: FieldElement,
    pub sequencer_address: FieldElement,
}
