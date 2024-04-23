use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;
use sozo_ops::model;
use starknet::core::types::FieldElement;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct ModelArgs {
    #[command(subcommand)]
    command: ModelCommand,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommand {
    #[command(about = "Retrieve the class hash of a model")]
    ClassHash {
        #[arg(help = "The name of the model")]
        name: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Retrieve the contract address of a model")]
    ContractAddress {
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
        let env_metadata = utils::load_metadata_from_config(config)?;

        config.tokio_handle().block_on(async {
            match self.command {
                ModelCommand::ClassHash { name, starknet, world } => {
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_class_hash(name, world_address, provider).await
                }
                ModelCommand::ContractAddress { name, starknet, world } => {
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_contract_address(name, world_address, provider).await
                }
                ModelCommand::Schema { name, to_json, starknet, world } => {
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_schema(name, world_address, provider, to_json).await
                }
                ModelCommand::Get { name, keys, starknet, world } => {
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_get(name, keys, world_address, provider).await
                }
            }
        })
    }
}