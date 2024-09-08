//! Data availability options.

use clap::Args;
use dojo_utils::env::{
    DOJO_ACCOUNT_ADDRESS_ENV_VAR, DOJO_KEYSTORE_PASSWORD_ENV_VAR, DOJO_KEYSTORE_PATH_ENV_VAR,
    DOJO_PRIVATE_KEY_ENV_VAR, STARKNET_RPC_URL_ENV_VAR,
};
use katana_primitives::felt::FieldElement;
use url::Url;

#[derive(Debug, Args, Clone)]
pub struct StarknetAccountOptions {
    #[arg(long, env = STARKNET_RPC_URL_ENV_VAR)]
    #[arg(help = "The url of the starknet node.")]
    pub starknet_url: Url,

    #[arg(long)]
    #[arg(env)]
    #[arg(help = "The chain id of the starknet node.")]
    pub chain_id: String,

    #[arg(long, env = DOJO_ACCOUNT_ADDRESS_ENV_VAR)]
    #[arg(help = "The address of the starknet account.")]
    pub signer_address: FieldElement,

    #[arg(long, env = DOJO_PRIVATE_KEY_ENV_VAR)]
    #[arg(help = "The private key of the starknet account.")]
    pub signer_key: Option<FieldElement>,

    #[arg(long = "keystore", env = DOJO_KEYSTORE_PATH_ENV_VAR)]
    #[arg(value_name = "PATH")]
    #[arg(help = "The path to the keystore file.")]
    pub signer_keystore_path: Option<String>,

    #[arg(long = "password", env = DOJO_KEYSTORE_PASSWORD_ENV_VAR)]
    #[arg(value_name = "PASSWORD")]
    #[arg(help = "The password to the keystore file.")]
    pub signer_keystore_password: Option<String>,
}
