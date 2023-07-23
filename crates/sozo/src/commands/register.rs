use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;
use starknet::core::types::FieldElement;

use super::options::account::AccountOptions;
use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::ops::register;

#[derive(Debug, Args)]
pub struct RegisterArgs {
    #[command(subcommand)]
    pub command: RegisterCommand,
}

#[derive(Debug, Subcommand)]
pub enum RegisterCommand {
    #[command(about = "Register a component to a world.")]
    Component {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "CLASS_HASH")]
        #[arg(help = "The class hash of the components to register.")]
        components: Vec<FieldElement>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,
    },

    #[command(about = "Register a system to a world.")]
    System {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "CLASS_HASH")]
        #[arg(help = "The class hash of the systems to register.")]
        systems: Vec<FieldElement>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,
    },
}

impl RegisterArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
            let env_metadata = dojo_metadata_from_workspace(&ws)
                .and_then(|dojo_metadata| dojo_metadata.get("env").cloned());

            env_metadata
                .as_ref()
                .and_then(|env_metadata| env_metadata.get(ws.config().profile().as_str()).cloned())
                .or(env_metadata)
        } else {
            None
        };

        config.tokio_handle().block_on(register::execute(self.command, env_metadata))
    }
}
