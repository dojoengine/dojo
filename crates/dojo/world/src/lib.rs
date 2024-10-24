#[cfg(feature = "metadata")]
pub mod config;
#[cfg(feature = "manifest")]
pub mod manifest;
#[cfg(feature = "metadata")]
pub mod metadata;
#[cfg(feature = "migration")]
pub mod migration;
#[cfg(feature = "metadata")]
pub mod uri;

type DojoSelector = starknet::core::types::Felt;
type Namespace = String;

pub mod contracts;
pub mod diff;
pub mod local;
pub mod remote;

#[cfg(test)]
pub mod test_utils;
