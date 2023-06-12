use anyhow::Result;
use dojo_world::world::WorldContractReader;
use starknet::core::types::{BlockId, BlockTag};
use toml::Value;

use crate::commands::component::ComponentCommands;

pub async fn execute(command: ComponentCommands, env_metadata: Option<Value>) -> Result<()> {
    match command {
        ComponentCommands::Get { name, world, starknet } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let component = world.component(&name, BlockId::Tag(BlockTag::Pending)).await?;

            println!("{:#x}", component.class_hash());
        }

        ComponentCommands::Schema { name, world, starknet, to_json } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let component = world.component(&name, BlockId::Tag(BlockTag::Pending)).await?;

            let schema = component.schema(BlockId::Tag(BlockTag::Pending)).await?;

            if to_json {
                println!("{}", serde_json::to_string_pretty(&schema)?)
            } else {
                let output = format!(
                    r"struct {name} {{
{}
}}",
                    schema
                        .iter()
                        .map(|s| format!(r"   {}: {}", s.name, s.ty))
                        .collect::<Vec<String>>()
                        .join("\n")
                );

                println!("{output}")
            }
        }

        ComponentCommands::Entity { name, partition_id, keys, starknet, world, .. } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let component = world.component(&name, BlockId::Tag(BlockTag::Pending)).await?;

            let entity =
                component.entity(partition_id, keys, BlockId::Tag(BlockTag::Pending)).await?;

            println!(
                "{}",
                entity.iter().map(|f| format!("{f:#x}")).collect::<Vec<String>>().join(" ")
            )
        }
    }

    Ok(())
}
