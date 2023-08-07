use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;

use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::ops::system;

#[derive(Debug, Args)]
pub struct SystemArgs {
    #[command(subcommand)]
    command: SystemCommands,
}

#[derive(Debug, Subcommand)]
pub enum SystemCommands {
    #[command(about = "Get the class hash of a system.")]
    Get {
        #[arg(help = "The name of the system.")]
        name: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(alias = "dep")]
    #[command(about = "Retrieve the component dependencies of a system.")]
    Dependency {
        #[arg(help = "The name of the system.")]
        name: String,

        #[arg(short = 'j', long = "json")]
        #[arg(help_heading = "Display options")]
        to_json: bool,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },
}

impl SystemArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            // TODO: Check the updated scarb way to read profile specific values
            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(system::execute(self.command, env_metadata))
    }
}
