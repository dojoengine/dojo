use clap::Args;
use katana_primitives::Felt;
use url::Url;

#[derive(Debug, Args, Clone)]
pub struct ProofOptions {
    #[arg(help = "The address of the World contract.")]
    #[arg(long = "world")]
    pub world_address: Felt,

    #[arg(help = "The address of the Fact Registry contract.")]
    #[arg(long = "registry")]
    pub fact_registry_address: Felt,

    #[arg(long)]
    #[arg(value_name = "PROVER URL")]
    #[arg(help = "The Prover URL for remote proving.")]
    pub prover_url: Url,

    #[arg(long)]
    #[arg(value_name = "PROVER KEY")]
    #[arg(help = "An authorized prover key for remote proving.")]
    pub private_key: String,
}
