use anyhow::Result;
use console::Style;
use dojo_world::metadata::Environment;
use starknet::core::types::{BlockId, BlockTag};
use torii_client::contract::world::WorldContractReader;

use crate::commands::system::SystemCommands;

pub async fn execute(command: SystemCommands, env_metadata: Option<Environment>) -> Result<()> {
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
                    Style::from_dotted_str("bold.underlined").apply_to("Read:"),
                    read.join("\n"),
                    Style::from_dotted_str("bold.underlined").apply_to("Write:"),
                    write.join("\n"),
                );

                println!("{output}")
            }
        }
    }

    Ok(())
}
