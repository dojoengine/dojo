use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;

use super::options::account::AccountOptions;
use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
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
        #[arg(help = "Name of the component to grant write access to.")]
        component: String,

        #[arg(help = "Name of the system to grant writer access to.")]
        system: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,
    },
}

impl AuthArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
            let env_metadata =
                dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned());
            // TODO: Check the updated scarb way to read profile specific values
            env_metadata
        } else {
            None
        };

        config.tokio_handle().block_on(auth::execute(self.command, env_metadata))
    }
}
