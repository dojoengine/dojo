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

pub type DojoSelector = starknet::core::types::Felt;
pub type Namespace = String;

#[derive(Debug, PartialEq)]
pub enum ResourceType {
    Namespace,
    Contract,
    Model,
    Event,
    StarknetContract,
}
