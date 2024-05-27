//! Prover backends.
//!
//! The prover is in charge of generating a proof from the cairo execution trace.
use std::str::FromStr;

use anyhow::bail;
use async_trait::async_trait;

mod client;
mod extract;
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
    Http(Box<HttpProverParams>),
}

pub enum ProveProgram {
    Differ,
    Merger,
}

pub async fn prove_diff(input: String, prover: ProverIdentifier) -> anyhow::Result<String> {
    match prover {
        ProverIdentifier::Http(params) => http_prove(*params, input, ProveProgram::Differ).await,
        ProverIdentifier::Stone => prove_stone(input).await,
        ProverIdentifier::Sharp => todo!(),
        ProverIdentifier::Platinum => todo!(),
    }
}

pub async fn prove_merge(input: String, prover: ProverIdentifier) -> anyhow::Result<String> {
    match prover {
        ProverIdentifier::Http(params) => http_prove(*params, input, ProveProgram::Merger).await,
        ProverIdentifier::Stone => prove_merge_stone(input).await,
        ProverIdentifier::Sharp => todo!(),
        ProverIdentifier::Platinum => todo!(),
    }
}

/// The prover client. in charge of producing the proof.
#[async_trait]
pub trait ProverClient {
    fn identifier() -> ProverIdentifier;

    /// Generates the proof from the given trace.
    /// At the moment prover is coupled with the program it proves. Because of this input should
    /// correspond to the program.
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
