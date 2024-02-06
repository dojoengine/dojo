//! Prover backends.
//!
//! The prover is in charge of generating a proof from the cairo execution trace.
use async_trait::async_trait;

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
}
