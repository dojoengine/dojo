//! Prover backends.
//!
//! The prover is in charge of generating a proof from the cairo execution trace.
use async_trait::async_trait;

mod serializer;
mod stone_image;
mod vec252;

pub use serializer::parse_proof;
pub use stone_image::StoneProver;

/// The prover used to generate the proof.
#[derive(Debug)]
pub enum ProverIdentifier {
    Sharp,
    Stone,
    Platinum,
}

/// The prover client. in charge of producing the proof.
#[async_trait]
pub trait ProverClient {
    fn identifier() -> ProverIdentifier;

    /// Setups the prover, this is a one time operation.
    async fn setup(&self, source: &str) -> anyhow::Result<()>;

    /// Generates the proof from the given trace.
    async fn prove(&self, input: String) -> anyhow::Result<String>;
    async fn local_verify(proof: String) -> anyhow::Result<()>;
}
