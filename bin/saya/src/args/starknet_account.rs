//! Data availability options.

use clap::Args;
use katana_primitives::felt::FieldElement;
use url::Url;

#[derive(Debug, Args, Clone)]
pub struct StarknetAccountOptions {
    #[arg(long)]
    #[arg(help = "The url of the starknet node.")]
    pub starknet_url: Url,

    #[arg(long)]
    #[arg(help = "The chain id of the starknet node.")]
    #[arg(default_value = "SN_SEPOLIA")]
    pub chain_id: String,

    #[arg(long)]
    #[arg(help = "The address of the starknet account.")]
    pub signer_address: FieldElement,

    #[arg(long)]
    #[arg(help = "The private key of the starknet account.")]
    pub signer_key: FieldElement,
}
