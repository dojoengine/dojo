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
use starknet::accounts::Call;
use starknet_crypto::FieldElement;
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

pub enum ProveDiffProgram {
    Differ,
    Merger,
}

pub enum ProveProgram {
    DiffProgram(ProveDiffProgram),
    Checker, // Contract specific checker program.
    Batcher, // Simulating snos, contract from dojo-os repository.
}

impl ProverIdentifier {
    pub async fn prove_diff(
        &self,
        input: String,
        program: ProveDiffProgram,
    ) -> anyhow::Result<String> {
        let program = ProveProgram::DiffProgram(program);

        match self {
            ProverIdentifier::Http(params) => {
                http_prove(params.clone(), input, program, false).await
            }
            ProverIdentifier::Stone => prove_stone(input, program).await,
            ProverIdentifier::Sharp => todo!(),
            ProverIdentifier::Platinum => todo!(),
        }
    }

    pub async fn prove_checker(&self, calls: Vec<Call>) -> anyhow::Result<String> {
        let len = FieldElement::from(calls.len() as u64);
        let args = calls
            .into_iter()
            .map(|c| {
                let mut felts = vec![c.to, c.selector, c.calldata.len().into()];
                felts.extend(c.calldata);
                felts
            })
            .flatten()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let input = format!("[{} {}]", len, args);

        match self {
            ProverIdentifier::Http(params) => {
                http_prove(params.clone(), input, ProveProgram::Checker, true).await
            }
            ProverIdentifier::Stone => todo!(),
            ProverIdentifier::Sharp => todo!(),
            ProverIdentifier::Platinum => todo!(),
        }
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
