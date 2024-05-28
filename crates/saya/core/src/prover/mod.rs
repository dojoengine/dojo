//! Prover backends.
//!
//! The prover is in charge of generating a proof from the cairo execution trace.
use std::str::FromStr;
use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;

mod client;
pub mod extract;
mod loader;
mod program_input;
mod scheduler;
pub mod state_diff;
mod stone_image;
mod vec252;

pub use client::HttpProverParams;
pub use program_input::*;
pub use scheduler::*;
pub use stone_image::*;

use self::client::http_prove;

/// The prover used to generate the proof.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProverIdentifier {
    #[default]
    Stone,
    Sharp,
    Platinum,
    Http(Arc<HttpProverParams>),
}

pub enum ProveProgram {
    Differ,
    Merger,
}

pub async fn prove_diff(
    input: String,
    prover: ProverIdentifier,
    program: ProveProgram,
) -> anyhow::Result<String> {
    match prover {
        ProverIdentifier::Http(params) => http_prove(params, input, program).await,
        ProverIdentifier::Stone => prove_stone(input, program).await,
        ProverIdentifier::Sharp => todo!(),
        ProverIdentifier::Platinum => todo!(),
    }
}

/// The prover client. in charge of producing the proof.
#[async_trait]
pub trait ProverClient {
    fn identifier() -> ProverIdentifier;

    /// Generates the proof from the given trace.
    /// The proven input has to be valid for the proving program.
    async fn prove(&self, input: String) -> anyhow::Result<String>;
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
