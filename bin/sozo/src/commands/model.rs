use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;
use sozo_ops::model;
use sozo_ops::utils::get_default_namespace_from_ws;
use starknet::core::types::FieldElement;
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
    #[command(about = "Retrieve the class hash of a model")]
    ClassHash {
        #[arg(help = "The name of the model")]
        name: String,

        #[arg(short, long)]
        #[arg(help = "The model namespace. If not set, the main package ID is used.")]
        namespace: Option<String>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Retrieve the contract address of a model")]
    ContractAddress {
        #[arg(help = "The name of the model")]
        name: String,

        #[arg(short, long)]
        #[arg(help = "The model namespace. If not set, the main package ID is used.")]
        namespace: Option<String>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

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
        #[arg(help = "The name of the model")]
        name: String,

        #[arg(short, long)]
        #[arg(help = "The model namespace. If not set, the main package ID is used.")]
        namespace: Option<String>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Retrieve the schema for a model")]
    Schema {
        #[arg(help = "The name of the model")]
        name: String,

        #[arg(short, long)]
        #[arg(help = "The model namespace. If not set, the main package ID is used.")]
        namespace: Option<String>,

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

        #[arg(short, long)]
        #[arg(help = "The model namespace. If not set, the main package ID is used.")]
        namespace: Option<String>,

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
        trace!(args = ?self);
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let env_metadata = utils::load_metadata_from_config(config)?;
        let get_namespace = |ns: Option<String>| -> String {
            match ns {
                Some(x) => x,
                None => {
                    let default_namespace = get_default_namespace_from_ws(&ws);
                    println!("[default namespace: {}]", default_namespace);
                    default_namespace
                }
            }
        };

        config.tokio_handle().block_on(async {
            match self.command {
                ModelCommand::ClassHash { name, namespace, starknet, world } => {
                    let namespace = get_namespace(namespace);
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_class_hash(namespace, name, world_address, provider).await
                }
                ModelCommand::ContractAddress { name, namespace, starknet, world } => {
                    let namespace = get_namespace(namespace);
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_contract_address(namespace, name, world_address, provider).await
                }
                ModelCommand::Layout { name, namespace, starknet, world } => {
                    let namespace = get_namespace(namespace);
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_layout(namespace, name, world_address, provider).await
                }
                ModelCommand::Schema { name, namespace, to_json, starknet, world } => {
                    let namespace = get_namespace(namespace);
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_schema(namespace, name, world_address, provider, to_json).await
                }
                ModelCommand::Get { name, namespace, keys, starknet, world } => {
                    let namespace = get_namespace(namespace);
                    let world_address = world.address(env_metadata.as_ref()).unwrap();
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    model::model_get(namespace, name, keys, world_address, provider).await
                }
            }
        })
    }
}
