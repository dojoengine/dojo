use anyhow::Result;
use dojo_world::metadata::Environment;
use starknet::core::types::{BlockId, BlockTag};
use torii_client::contract::world::WorldContractReader;

use crate::commands::model::ModelCommands;

pub async fn execute(command: ModelCommands, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        ModelCommands::ClassHash { name, world, starknet } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let model = world.model(&name, BlockId::Tag(BlockTag::Pending)).await?;

            println!("{:#x}", model.class_hash());
        }

        ModelCommands::Schema { name, world, starknet, to_json } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let model = world.model(&name, BlockId::Tag(BlockTag::Pending)).await?;

            let schema = model.schema(BlockId::Tag(BlockTag::Pending)).await?;

            if to_json {
                println!("{}", serde_json::to_string_pretty(&schema)?)
            } else {
                println!("{schema}");
            }
        }

        ModelCommands::Entity { name, keys, starknet, world, .. } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let model = world.model(&name, BlockId::Tag(BlockTag::Pending)).await?;

            let entity = model.entity(keys, BlockId::Tag(BlockTag::Pending)).await?;

            println!(
                "{}",
                entity.iter().map(|f| format!("{f:#x}")).collect::<Vec<String>>().join("\n")
            )
        }
    }

    Ok(())
}
