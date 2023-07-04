use anyhow::Result;
use dojo_client::contract::world::WorldContractReader;
use starknet::core::types::{BlockId, BlockTag};
use toml::Value;
use yansi::Paint;

use crate::commands::system::SystemCommands;

pub async fn execute(command: SystemCommands, env_metadata: Option<Value>) -> Result<()> {
    match command {
        SystemCommands::Get { name, world, starknet } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let system = world.system(&name, BlockId::Tag(BlockTag::Pending)).await?;

            println!("{:#x}", system.class_hash())
        }

        SystemCommands::Dependency { name, to_json, world, starknet } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider);
            let system = world.system(&name, BlockId::Tag(BlockTag::Pending)).await?;

            let deps = system.dependencies(BlockId::Tag(BlockTag::Pending)).await?;

            if to_json {
                println!("{}", serde_json::to_string_pretty(&deps)?);
            } else {
                let read = deps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, d)| {
                        if d.read {
                            Some(format!("{}.{}", i + 1, d.name.clone()))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                let write = deps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, d)| {
                        if d.write {
                            Some(format!("{}. {}", i + 1, d.name.clone()))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                let output = format!(
                    r"{}
{}
{}
{}",
                    Paint::new("Read:").bold().underline(),
                    read.join("\n"),
                    Paint::new("Write:").bold().underline(),
                    write.join("\n"),
                );

                println!("{output}")
            }
        }
    }

    Ok(())
}
