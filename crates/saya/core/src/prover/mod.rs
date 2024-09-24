//! Prover backends.
//!
//! The prover is in charge of generating a proof from the cairo execution trace.
use std::str::FromStr;
use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;

mod client;
mod loader;
pub mod persistent;
mod program_input;
use cairo_proof_parser::to_felts;
use client::http_prove;
pub use client::HttpProverParams;
use persistent::BatcherInput;
pub use program_input::*;
use prover_sdk::ProverResult;
use starknet::accounts::Call;
use starknet_crypto::Felt;

use crate::error::ProverError;
// pub use stone_image::*;

/// The prover used to generate the proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProverIdentifier {
    HerodotusSharp,
    Http(Arc<HttpProverParams>),
}

#[derive(Debug)]
pub enum ProveProgram {
    Checker, // Contract specific checker program.
    Batcher, // Simulating snos, contract from dojo-os repository.
}

impl ProverIdentifier {
    pub async fn prove_checker(&self, calls: Vec<Call>) -> Result<ProverResult, ProverError> {
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
                http_prove(params.clone(), args, ProveProgram::Checker).await
            }
            ProverIdentifier::HerodotusSharp => todo!(),
        }
    }

    pub async fn prove_snos(&self, calls: BatcherInput) -> Result<ProverResult, ProverError> {
        let calldata = to_felts(&calls).map_err(|e| ProverError::SerdeFeltError(e.to_string()))?;

        match self {
            ProverIdentifier::Http(params) => {
                http_prove(params.clone(), calldata, ProveProgram::Batcher).await
            }
            ProverIdentifier::HerodotusSharp => todo!(),
        }
    }
}

impl ProveProgram {
    pub fn cairo_version(&self) -> Felt {
        match self {
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
            "herodotus-sharp" => ProverIdentifier::HerodotusSharp,
            _ => bail!("Unknown prover: `{}`.", prover),
        })
    }
}
