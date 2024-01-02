//! Translation layer for converting the primitive types to the execution engine types.

use starknet::core::utils::parse_cairo_short_string;
use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;

use crate::chain::ChainId;

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

impl From<ChainId> for starknet_api::core::ChainId {
    fn from(chain_id: ChainId) -> Self {
        match chain_id {
            ChainId::Named(named) => Self(named.to_string()),
            ChainId::Id(id) => Self(parse_cairo_short_string(&id).expect("valid cairo string")),
        }
    }
}
