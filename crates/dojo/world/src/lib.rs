#![cfg_attr(not(test), warn(unused_crate_dependencies))]

#[cfg(feature = "metadata")]
pub mod metadata;

pub mod config;
pub mod contracts;
pub mod diff;
pub mod local;
pub mod remote;
pub mod uri;
pub mod utils;

#[cfg(test)]
pub mod test_utils;

// To avoid depending on cainome types or other crate,
// those aliases are mostly for readability.
pub type DojoSelector = starknet::core::types::Felt;
pub type ContractAddress = starknet::core::types::Felt;

#[derive(Debug, PartialEq)]
pub enum ResourceType {
    Namespace,
    Contract,
    Model,
    Event,
    StarknetContract,
}
