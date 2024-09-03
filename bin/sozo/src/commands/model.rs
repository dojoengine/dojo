use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;
use sozo_ops::model;
use starknet::core::types::Felt;
use tracing::trace;

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
    #[command(about = "Displays the model's layout into dojo storage.\n
The Dojo storage system uses the poseidon_hash function to compute
hashes, called 'hash' in the following documentation.

        How storage locations are computed ?

        model key               = hash(model_keys)

        fixed layout key        = parent_key
        struct layout field key = hash(parent_key, field_selector)
        tuple layout item key   = hash(parent_key, item_index)
        enum layout
                    variant key = parent_key
                    data key    = hash(parent_key, variant_index)
        array layout
                    length key  = parent_key
                    item key    = hash(parent_key, item_index)
        byte array layout       = parent_key

        final storage location  = hash('dojo_storage', model_selector, record_key)")]
    Layout {
        #[arg(help = "The tag or name of the model")]
        tag_or_name: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Retrieve the schema for a model")]
    Schema {
        #[arg(help = "The tag or name of the model")]
        tag_or_name: String,

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
        #[arg(help = "The tag or name of the model")]
        tag_or_name: String,

        #[arg(value_name = "KEYS")]
        #[arg(value_delimiter = ',')]
        #[arg(help = "Comma seperated values e.g., 0x12345,0x69420,...")]
        keys: Vec<Felt>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },
}

impl ModelArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);
        let env_metadata = utils::load_metadata_from_config(config)?;

        config.tokio_handle().block_on(async {
            match self.command {
                ModelCommand::Layout { tag_or_name, starknet, world } => {
                    let tag = model::check_tag_or_read_default_namespace(&tag_or_name, config)?;

                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_layout(tag, world_address, provider).await?;
                    Ok(())
                }
                ModelCommand::Schema { tag_or_name, to_json, starknet, world } => {
                    let tag = model::check_tag_or_read_default_namespace(&tag_or_name, config)?;

                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_schema(tag, world_address, provider, to_json).await?;
                    Ok(())
                }
                ModelCommand::Get { tag_or_name, keys, starknet, world } => {
                    let tag = model::check_tag_or_read_default_namespace(&tag_or_name, config)?;

                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_get(tag, keys, world_address, provider).await?;
                    Ok(())
                }
            }
        })
    }
}
