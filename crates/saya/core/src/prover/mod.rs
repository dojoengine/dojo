//! Prover backends.
//!
//! The prover is in charge of generating a proof from the cairo execution trace.
use std::str::FromStr;

use anyhow::bail;
use async_trait::async_trait;

mod serializer;
pub mod state_diff;
mod stone_image;
mod vec252;

use serde::{Deserialize, Serialize};
pub use serializer::parse_proof;
pub use stone_image::*;

/// The prover used to generate the proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProverIdentifier {
    #[default]
    Stone,
    Sharp,
    Platinum,
}

pub async fn prove(input: String, prover: ProverIdentifier) -> anyhow::Result<String> {
    match prover {
        ProverIdentifier::Sharp => todo!(),
        ProverIdentifier::Stone => prove_stone(input).await,
        ProverIdentifier::Platinum => todo!(),
    }
}

/// The prover client. in charge of producing the proof.
#[async_trait]
pub trait ProverClient {
    fn identifier() -> ProverIdentifier;

    /// Generates the proof from the given trace.
    /// At the moment prover is coupled with the program it proves. Because of this input should correspond to the program.
    async fn prove(&self, input: String) -> anyhow::Result<String>;
    async fn local_verify(&self, proof: String) -> anyhow::Result<()>;
}

impl FromStr for ProverIdentifier {
    type Err = anyhow::Error;

    fn from_str(prover: &str) -> anyhow::Result<Self> {
        Ok(match prover {
            "stone" => ProverIdentifier::Stone,
            "sharp" => ProverIdentifier::Sharp,
            "platinum" => ProverIdentifier::Platinum,
            _ => bail!("Unknown prover: `{}`.", prover),
        })
    }
}
