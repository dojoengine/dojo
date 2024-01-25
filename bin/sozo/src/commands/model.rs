use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;
use starknet::core::types::FieldElement;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::ops::model;

#[derive(Debug, Args)]
pub struct ModelArgs {
    #[command(subcommand)]
    command: ModelCommands,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    #[command(about = "Retrieve the class hash of a model")]
    ClassHash {
        #[arg(help = "The name of the model")]
        name: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Retrieve the schema for a model")]
    Schema {
        #[arg(help = "The name of the model")]
        name: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[arg(short = 'j', long = "json")]
        #[arg(help_heading = "Display options")]
        to_json: bool,
    },

    #[command(about = "Get a models value for the provided key")]
    Get {
        #[arg(help = "The name of the model")]
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

impl ModelArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(model::execute(self.command, env_metadata))
    }
}
