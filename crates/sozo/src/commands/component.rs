use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;
use starknet::core::types::FieldElement;

use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::ops::component;

#[derive(Debug, Args)]
pub struct ComponentArgs {
    #[command(subcommand)]
    command: ComponentCommands,
}

#[derive(Debug, Subcommand)]
pub enum ComponentCommands {
    #[command(about = "Get the class hash of a component")]
    Get {
        #[arg(help = "The name of the component")]
        name: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Retrieve the schema for a component")]
    Schema {
        #[arg(help = "The name of the component")]
        name: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[arg(short = 'j', long = "json")]
        #[arg(help_heading = "Display options")]
        to_json: bool,
    },

    #[command(about = "Get the component value for an entity")]
    Entity {
        #[arg(help = "The name of the component")]
        name: String,

        #[arg(value_name = "KEYS")]
        #[arg(value_delimiter = ',')]
        #[arg(help = "Comma seperated values e.g., 0x12345,0x69420,...")]
        keys: Vec<FieldElement>,

        #[arg(long = "parition_id", default_value = "0x0")]
        #[arg(help = "Entity query parition id.")]
        partition_id: FieldElement,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },
}

impl ComponentArgs {
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

        config.tokio_handle().block_on(component::execute(self.command, env_metadata))
    }
}
