//! Data availability.
//!
//! For a starknet based sequencer, the data posted to the DA
//! is the state diff as encoded here:
//! https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/on-chain-data/#data_availability_v0_11_0_and_forward.
//!
use async_trait::async_trait;
use starknet::core::types::FieldElement;

pub mod state_diff;
pub mod error;
use error::DataAvailabilityResult;

/// The data availability mode.
#[derive(Debug)]
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
    ///
    /// # Arguments
    ///
    /// * `data` - An array of felt representing the data to be published
    ///   on the DA layer. We use felt as all fields inside the state diff
    ///   can be expressed as a felt. Nonce and updates count are limited to
    ///   64 bits anyway.
    async fn publish_data(data: &[FieldElement]) -> DataAvailabilityResult<()>;
}
