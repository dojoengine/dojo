//! Selecting prover and verifier.

use clap::Args;
use saya_core::prover::ProverIdentifier;
use saya_core::verifier::VerifierIdentifier;

#[derive(Debug, Args, Clone)]
pub struct ProverOptions {
    #[arg(long)]
    #[arg(help = "Data availability chain name")]
    pub prover: Option<ProverIdentifier>,

    #[arg(long)]
    #[arg(help = "Data availability chain name")]
    pub verifier: Option<VerifierIdentifier>,
}
