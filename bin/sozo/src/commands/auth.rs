use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::ops::auth;

#[derive(Debug, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    #[command(about = "Auth a system with the given calldata.")]
    Writer {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "model,contract_address")]
        #[arg(help = "A list of models and contract address to grant write access to. Comma \
                      separated values to indicate model name and contract address e.g. \
                      model_name,0x1234 model_name,0x1111 ")]
        models_contracts: Vec<String>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,

        #[command(flatten)]
        transaction: TransactionOptions,
    },
}

impl AuthArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(auth::execute(self.command, env_metadata))
    }
}
