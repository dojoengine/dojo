use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use dojo_world::config::calldata_decoder;
use scarb_interop::MetadataDojoExt;
use scarb_metadata::Metadata;
use sozo_ops::model;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use starknet::core::types::{BlockId, BlockTag, Felt};
use tracing::trace;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;
use crate::utils::CALLDATA_DOC;

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
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(
            help = format!("List of values representing the serialized keys of the model.\n{CALLDATA_DOC}")
        )]
        keys: Vec<String>,

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
    pub async fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        trace!(args = ?self);

        let profile_config = scarb_metadata.load_dojo_profile_config()?;
        let default_ns = profile_config.namespace.default;

        match self.command {
            ModelCommand::ClassHash { tag_or_name, starknet, world } => {
                let tag = tag_or_name.ensure_namespace(&default_ns);

                let (world_diff, provider, _) =
                    utils::get_world_diff_and_provider(starknet, world, &scarb_metadata).await?;

                model::model_class_hash(tag.to_string(), world_diff.world_info.address, &provider)
                    .await?;
                Ok(())
            }
            ModelCommand::ContractAddress { tag_or_name, starknet, world } => {
                let tag = tag_or_name.ensure_namespace(&default_ns);

                let (world_diff, provider, _) =
                    utils::get_world_diff_and_provider(starknet, world, &scarb_metadata).await?;

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
                    utils::get_world_diff_and_provider(starknet, world, &scarb_metadata).await?;

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
                    utils::get_world_diff_and_provider(starknet, world, &scarb_metadata).await?;

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
                    utils::get_world_diff_and_provider(starknet, world, &scarb_metadata).await?;

                let (record, _, _) = model::model_get(
                    tag.to_string(),
                    parse_keys(&keys)?,
                    world_diff.world_info.address,
                    &provider,
                    block_id,
                )
                .await?;

                println!("{}", record);

                Ok(())
            }
        }
    }
}

/// Parses the keys from the command line into a vector of Felt representing the serialized keys of
/// the model.
fn parse_keys(keys: &[String]) -> Result<Vec<Felt>> {
    let mut keys_serde = vec![];

    for key in keys {
        let key_felt = calldata_decoder::decode_single_calldata(key)
            .with_context(|| format!("Failed to decode key: {}", key))?;
        keys_serde.extend(key_felt);
    }

    Ok(keys_serde)
}

#[cfg(test)]
mod tests {
    // To do: Add more tests for the flattening of keys
    // let flattened_keys: Vec<Felt> = keys.into_iter().flatten().collect();

    use clap::Parser;
    use starknet::core::utils::cairo_short_string_to_felt;
    use starknet::macros::felt;

    use super::*;

    #[derive(Parser, Debug)]
    struct TestCommand {
        #[command(subcommand)]
        command: ModelCommand,
    }

    #[test]
    fn test_model_get_argument_parsing() {
        // Test parsing with hex
        let args = TestCommand::parse_from([
            "model",
            "get",
            "Account",
            "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5",
            "0x6d69737479",
        ]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![
                felt!("0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5"),
                felt!("0x6d69737479"),
            ];

            assert_eq!(parse_keys(&keys).unwrap(), expected);
        } else {
            panic!("Expected Get command");
        }

        // Test parsing with short string prefix
        let args = TestCommand::parse_from([
            "model",
            "get",
            "Account",
            "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5",
            "sstr:\"misty\"",
        ]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![
                felt!("0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5"),
                cairo_short_string_to_felt("misty").unwrap(),
            ];

            assert_eq!(parse_keys(&keys).unwrap(), expected);
        } else {
            panic!("Expected Get command");
        }

        // Test parsing with u256 prefix
        let args = TestCommand::parse_from([
            "model",
            "get",
            "Account",
            "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5",
            "u256:0x1",
        ]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![
                felt!("0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5"),
                Felt::ONE,
                Felt::ZERO,
            ];

            assert_eq!(parse_keys(&keys).unwrap(), expected);
        } else {
            panic!("Expected Get command");
        }

        // Test parsing with int prefix
        let args = TestCommand::parse_from(["model", "get", "Account", "int:-123456789"]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![(-123456789_i64).into()];

            assert_eq!(parse_keys(&keys).unwrap(), expected);
        } else {
            panic!("Expected Get command");
        }

        // Test parsing with str prefix
        let args = TestCommand::parse_from(["model", "get", "Account", "str:hello"]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![
                Felt::ZERO,
                cairo_short_string_to_felt("hello").unwrap(),
                Felt::from_dec_str("5").unwrap(),
            ];

            assert_eq!(parse_keys(&keys).unwrap(), expected);
        } else {
            panic!("Expected Get command");
        }

        // Test parsing with all prefixes
        let args = TestCommand::parse_from([
            "model",
            "get",
            "Account",
            "0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5",
            "u256:0x1",
            "int:-123456789",
            "str:hello",
        ]);

        if let ModelCommand::Get { keys, .. } = args.command {
            let expected = vec![
                felt!("0x054cb935d86d80b5a0a6e756edf448ab33876d01dd2b07a2a4e63a41e06d0ef5"),
                Felt::ONE,
                Felt::ZERO,
                (-123456789_i64).into(),
                Felt::ZERO,
                cairo_short_string_to_felt("hello").unwrap(),
                Felt::from_dec_str("5").unwrap(),
            ];

            assert_eq!(parse_keys(&keys).unwrap(), expected);
        } else {
            panic!("Expected Get command");
        }
    }
}
