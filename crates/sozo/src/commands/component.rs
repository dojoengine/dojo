use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::environment::dojo_metadata_from_workspace;
use scarb::core::Config;
use starknet::core::types::FieldElement;

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

            // TODO: Check the updated scarb way to read profile specific values
            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(component::execute(self.command, env_metadata))
    }
}
