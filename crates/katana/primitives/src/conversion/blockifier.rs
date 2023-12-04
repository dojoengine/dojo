//! Translation layer for converting the primitive types to the execution engine types.

use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;

impl From<crate::contract::ContractAddress> for ContractAddress {
    fn from(address: crate::contract::ContractAddress) -> Self {
        Self(patricia_key!(address.0))
    }
}

impl From<ContractAddress> for crate::contract::ContractAddress {
    fn from(address: ContractAddress) -> Self {
        Self((*address.0.key()).into())
    }
}
