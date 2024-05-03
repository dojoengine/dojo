use clap::Args;
use katana_primitives::FieldElement;

#[derive(Default, Debug, Args, Clone)]
pub struct ProofOptions {
    #[arg(help = "The address of the World contract.")]
    #[arg(long = "world")]
    pub world_address: FieldElement,

    #[arg(help = "The address of the Fact Registry contract.")]
    #[arg(long = "registry")]
    pub fact_registry_address: FieldElement,
}
