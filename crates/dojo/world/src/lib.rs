#[cfg(feature = "metadata")]
pub mod metadata;

pub mod config;
pub mod contracts;
pub mod diff;
pub mod local;
pub mod remote;
pub mod uri;

#[cfg(test)]
pub mod test_utils;

type DojoSelector = starknet::core::types::Felt;
type Namespace = String;
