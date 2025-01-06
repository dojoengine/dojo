use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::config::calldata_decoder;
use scarb::core::Config;
use sozo_ops::model;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use sozo_scarbext::WorkspaceExt;
use starknet::core::types::{BlockId, BlockTag, Felt};
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
        #[arg(help = "The tag or name of the model")]
        tag_or_name: ResourceDescriptor,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Retrieve the contract address of a model")]
    ContractAddress {
        #[arg(help = "The tag or name of the model")]
        tag_or_name: ResourceDescriptor,

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
        #[arg(help = "The tag or name of the model")]
        tag_or_name: ResourceDescriptor,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[arg(short, long)]
        #[arg(
            help = "Block number at which to retrieve the model layout (pending block by default)"
        )]
        block: Option<u64>,
    },

    #[command(about = "Retrieve the schema for a model")]
    Schema {
        #[arg(help = "The tag or name of the model")]
        tag_or_name: ResourceDescriptor,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[arg(short = 'j', long = "json")]
        #[arg(help_heading = "Display options")]
        to_json: bool,

        #[arg(short, long)]
        #[arg(
            help = "Block number at which to retrieve the model schema (pending block by default)"
        )]
        block: Option<u64>,
    },

    #[command(about = "Get a models value for the provided key")]
    Get {
        #[arg(help = "The tag or name of the model")]
        tag_or_name: ResourceDescriptor,

        #[arg(value_name = "KEYS")]
        #[arg(value_delimiter = ',')]
        #[arg(help = "Comma separated values e.g., \
                     0x12345,0x69420,sstr:\"hello\",sstr:\"misty\". Supported prefixes:\n  \
                     - sstr: A cairo short string\n  \
                     - no prefix: A cairo felt")]
        keys: String,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[arg(short, long)]
        #[arg(
            help = "Block number at which to retrieve the model data (pending block by default)"
        )]
        block: Option<u64>,
    },
}

impl ModelArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let profile_config = ws.load_profile_config()?;
        let default_ns = profile_config.namespace.default;

        config.tokio_handle().block_on(async {
            match self.command {
                ModelCommand::ClassHash { tag_or_name, starknet, world } => {
                    let tag = tag_or_name.ensure_namespace(&default_ns);

                    let (world_diff, provider, _) =
                        utils::get_world_diff_and_provider(starknet, world, &ws).await?;

                    model::model_class_hash(
                        tag.to_string(),
                        world_diff.world_info.address,
                        &provider,
                    )
                    .await?;
                    Ok(())
                }
                ModelCommand::ContractAddress { tag_or_name, starknet, world } => {
                    let tag = tag_or_name.ensure_namespace(&default_ns);

                    let (world_diff, provider, _) =
                        utils::get_world_diff_and_provider(starknet, world, &ws).await?;

                    model::model_contract_address(
                        tag.to_string(),
                        world_diff.world_info.address,
                        &provider,
                    )
                    .await?;
                    Ok(())
                }
                ModelCommand::Layout { tag_or_name, starknet, world, block } => {
                    let tag = tag_or_name.ensure_namespace(&default_ns);
                    let block_id =
                        block.map(BlockId::Number).unwrap_or(BlockId::Tag(BlockTag::Pending));

                    let (world_diff, provider, _) =
                        utils::get_world_diff_and_provider(starknet, world, &ws).await?;

                    model::model_layout(
                        tag.to_string(),
                        world_diff.world_info.address,
                        &provider,
                        block_id,
                    )
                    .await?;
                    Ok(())
                }
                ModelCommand::Schema { tag_or_name, to_json, starknet, world, block } => {
                    let tag = tag_or_name.ensure_namespace(&default_ns);
                    let block_id =
                        block.map(BlockId::Number).unwrap_or(BlockId::Tag(BlockTag::Pending));

                    let (world_diff, provider, _) =
                        utils::get_world_diff_and_provider(starknet, world, &ws).await?;

                    model::model_schema(
                        tag.to_string(),
                        world_diff.world_info.address,
                        &provider,
                        block_id,
                        to_json,
                    )
                    .await?;
                    Ok(())
                }
                ModelCommand::Get { tag_or_name, keys, block, starknet, world } => {
                    let tag = tag_or_name.ensure_namespace(&default_ns);
                    let block_id =
                        block.map(BlockId::Number).unwrap_or(BlockId::Tag(BlockTag::Pending));

                    let (world_diff, provider, _) =
                        utils::get_world_diff_and_provider(starknet, world, &ws).await?;

                    let keys = calldata_decoder::decode_calldata(&keys)?;

                    let (record, _, _) = model::model_get(
                        tag.to_string(),
                        keys,
                        world_diff.world_info.address,
                        &provider,
                        block_id,
                    )
                    .await?;

                    println!("{}", record);

                    Ok(())
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starknet::core::utils::cairo_short_string_to_felt;

    #[test]
    fn test_short_string_equals_felt() {
        // Test that sstr:"misty" equals 0x6d69737479
        let with_prefix = "sstr:\"misty\"";
        let with_hex = "0x6d69737479";

        let felt_from_string = calldata_decoder::decode_calldata(with_prefix).unwrap();
        let felt_from_hex = calldata_decoder::decode_calldata(with_hex).unwrap();

        assert_eq!(felt_from_string, felt_from_hex);
        assert_eq!(felt_from_string[0], Felt::from_hex_str("0x6d69737479").unwrap());
    }

    #[test]
    fn test_hex_equals_decimal() {
        // Test that 0x6d69737479 equals 469920609401
        let with_hex = "0x6d69737479";
        let with_decimal = "469920609401";

        let felt_from_hex = calldata_decoder::decode_calldata(with_hex).unwrap();
        let felt_from_decimal = calldata_decoder::decode_calldata(with_decimal).unwrap();

        assert_eq!(felt_from_hex, felt_from_decimal);
        assert_eq!(felt_from_hex[0], Felt::from(469920609401u128));
    }
}
