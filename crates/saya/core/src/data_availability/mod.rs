//! Data availability.
//!
//! For a starknet based sequencer, the data posted to the DA
//! is the state diff as encoded here:
//! <https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/on-chain-data/#data_availability_v0_11_0_and_forward>.
use std::fmt::Display;

use async_trait::async_trait;
use celestia_types::Commitment;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

pub mod celestia;

pub mod error;
use error::DataAvailabilityResult;

use crate::prover::persistent::PublishedStateDiff;

/// All possible chains configuration for data availability.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DataAvailabilityConfig {
    Celestia(celestia::CelestiaConfig),
}

impl Display for DataAvailabilityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataAvailabilityConfig::Celestia(conf) => {
                write!(f, "chain: celestia\n{conf}")
            }
        }
    }
}

/// The data availability mode.
#[derive(Debug, Copy, Clone)]
pub enum DataAvailabilityMode {
    /// The data are posted on the verification layer.
    Rollup,
    /// The data are posted off-chain (not the verification layer).
    Validium,
    /// The data are posted using one of rollup or validium, at the
    /// transaction level.
    Volition,
}

/// The data availbility client in charge
/// of interacting with the DA layer.
#[async_trait]
pub trait DataAvailabilityClient {
    /// Retrieves the client's DA mode.
    fn mode(&self) -> DataAvailabilityMode;

    /// Publishes data on the DA layer.
    /// Returns the block height in which the state diff was included.
    ///
    /// # Arguments
    ///
    /// * `state_diff` - An array of felt representing the data to be published on the DA layer. We
    ///   use felt as all fields inside the state diff can be expressed as a felt. Nonce and updates
    ///   count are limited to 64 bits anyway.
    async fn publish_state_diff_felts(&self, state_diff: &[Felt]) -> DataAvailabilityResult<(Commitment, u64)>;

    /// Publishes both data and transition proof on the DA layer atomically.
    /// Returns the block height in which the state diff was included.
    ///
    /// # Arguments
    ///
    /// * `state_diff` - An array of felt representing the data to be published on the DA layer. We
    ///   use felt as all fields inside the state diff can be expressed as a felt. Nonce and updates
    ///   count are limited to 64 bits anyway.
    ///  * `state_diff_proof` - The serialized transition proof corresponding to the `state_diff`.
    async fn publish_state_diff_and_proof_felts(
        &self,
        state_diff: &[Felt],
        state_diff_proof: &[Felt],
    ) -> DataAvailabilityResult<(Commitment, u64)>;

    /// Publishes a JSON-formatted proof on the DA layer.
    /// Returns the block height in which the proof was included.
    ///
    /// # Arguments
    ///
    /// * `state_diff` - A JSON string representing the proof to be published.
    async fn publish_checkpoint(
        &self,
        state_diff: PublishedStateDiff,
    ) -> DataAvailabilityResult<(Commitment, u64)>;
}

/// Initializes a [`DataAvailabilityClient`] from a [`DataAvailabilityConfig`].
///
/// # Arguments
///
/// * `config` - The data availability configuration.
pub async fn client_from_config(
    config: DataAvailabilityConfig,
) -> DataAvailabilityResult<Box<dyn DataAvailabilityClient>> {
    match config {
        DataAvailabilityConfig::Celestia(c) => {
            Ok(Box::new(celestia::CelestiaClient::new(c).await?))
        }
    }
}
