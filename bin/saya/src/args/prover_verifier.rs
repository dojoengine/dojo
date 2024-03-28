//! Selecting prover and verifier.

use clap::Args;
use saya_core::prover::ProverIdentifier;
use saya_core::verifier::VerifierIdentifier;

#[derive(Debug, Args, Clone)]
pub struct ProverOptions {
    #[arg(long)]
    #[arg(help = "Prover to be used <stone|sharp|platinum>")]
    pub prover: Option<ProverIdentifier>,

    #[arg(long)]
    #[arg(help = "Veryfier to be used <herodotus|local|starkware>")]
    pub verifier: Option<VerifierIdentifier>,
}
