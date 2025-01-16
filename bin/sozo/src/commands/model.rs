use anyhow::Result;
use clap::{value_parser, Args, Subcommand};
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
                     0x12345,0x69420,sstr:\"hello\". Supported prefixes:\n  \
                     - sstr: A cairo short string\n  \
                     - no prefix: A cairo felt")]
        #[arg(value_parser = model_key_parser)]
        keys: Vec<Felt>,

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

// Custom parser for model keys
fn model_key_parser(s: &str) -> Result<Felt> {
    if s.contains(':') && !s.starts_with("sstr:") {
        anyhow::bail!("Only 'sstr:' prefix is supported for model keys");
    }
    let felts = calldata_decoder::decode_calldata(s)?;
    Ok(felts[0])
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
    use clap::Parser;
    use starknet::core::utils::cairo_short_string_to_felt;

    #[derive(Parser, Debug)]
    struct TestCommand {
        #[command(subcommand)]
        command: ModelCommand,
    }

    #[test]
    fn test_model_get_argument_parsing() {
        // Test parsing with hex
        let args = TestCommand::parse_from([
            "model", // Placeholder for the binary name
            "get",
            "Account",
            "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5,0x6d69737479",
        ]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![
                Felt::from_hex(
                    "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5",
                )
                .unwrap(),
                Felt::from_hex("0x6d69737479").unwrap(),
            ];
            assert_eq!(keys, expected);
        } else {
            panic!("Expected Get command");
        }

        // Test parsing with string prefix
        let args = TestCommand::parse_from([
            "model",
            "get",
            "Account",
            "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5,sstr:\"misty\"",
        ]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![
                Felt::from_hex(
                    "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5",
                )
                .unwrap(),
                cairo_short_string_to_felt("misty").unwrap(),
            ];
            assert_eq!(keys, expected);
        } else {
            panic!("Expected Get command");
        }

        // Test invalid prefix
        let result =
            TestCommand::try_parse_from(["model", "get", "Account", "0x123,str:\"hello\""]);
        assert!(result.is_err());
    }
}
