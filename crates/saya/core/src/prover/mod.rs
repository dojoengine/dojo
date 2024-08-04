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
pub mod persistent;
mod program_input;
mod scheduler;
pub mod state_diff;
mod stone_image;
mod vec252;

use cairo_proof_parser::to_felts;
use client::http_prove_felts;
pub use client::HttpProverParams;
use persistent::BatcherInput;
pub use program_input::*;
pub use scheduler::*;
use starknet::accounts::Call;
use starknet_crypto::Felt;
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
            ProverIdentifier::Http(params) => http_prove(params.clone(), input, program).await,
            ProverIdentifier::Stone => prove_stone(input, program).await,
            ProverIdentifier::Sharp => todo!(),
            ProverIdentifier::Platinum => todo!(),
        }
    }

    pub async fn prove_checker(&self, calls: Vec<Call>) -> anyhow::Result<String> {
        let len = Felt::from(calls.len() as u64);
        let mut args = calls
            .into_iter()
            .flat_map(|c| {
                let mut felts = vec![c.to, c.selector, c.calldata.len().into()];
                felts.extend(c.calldata);
                felts
            })
            .collect::<Vec<_>>();
        args.insert(0, len);

        match self {
            ProverIdentifier::Http(params) => {
                http_prove_felts(params.clone(), args, ProveProgram::Checker).await
            }
            ProverIdentifier::Stone => todo!(),
            ProverIdentifier::Sharp => todo!(),
            ProverIdentifier::Platinum => todo!(),
        }
    }

    pub async fn prove_snos(&self, calls: BatcherInput) -> anyhow::Result<String> {
        let calldata = to_felts(&calls)?;

        match self {
            ProverIdentifier::Http(params) => {
                http_prove_felts(params.clone(), calldata, ProveProgram::Batcher).await
            }
            ProverIdentifier::Stone => todo!(),
            ProverIdentifier::Sharp => todo!(),
            ProverIdentifier::Platinum => todo!(),
        }
    }
}

impl ProveProgram {
    pub fn cairo_version(&self) -> Felt {
        match self {
            ProveProgram::DiffProgram(_) => Felt::ZERO,
            ProveProgram::Checker => Felt::ONE,
            ProveProgram::Batcher => Felt::ONE,
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
