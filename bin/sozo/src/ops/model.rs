use anyhow::Result;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use dojo_world::metadata::Environment;
use starknet::core::types::{BlockId, BlockTag};

use crate::commands::model::ModelCommands;

pub async fn execute(command: ModelCommands, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        ModelCommands::ClassHash { name, world, starknet } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider)
                .with_block(BlockId::Tag(BlockTag::Pending));

            let model = world.model_reader(&name).await?;

            println!("{:#x}", model.class_hash());
        }

        ModelCommands::ContractAddress { name, world, starknet } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider)
                .with_block(BlockId::Tag(BlockTag::Pending));

            let model = world.model_reader(&name).await?;

            println!("{:#x}", model.contract_address());
        }

        ModelCommands::Schema { name, world, starknet, to_json } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world: WorldContractReader<
                &starknet::providers::JsonRpcClient<starknet::providers::jsonrpc::HttpTransport>,
            > = WorldContractReader::new(world_address, &provider)
                .with_block(BlockId::Tag(BlockTag::Pending));

            let model = world.model_reader(&name).await?;
            let schema = model.schema().await?;

            if to_json {
                println!("{}", serde_json::to_string_pretty(&schema)?)
            } else {
                println!("{schema}");
            }
        }

        ModelCommands::Get { name, keys, starknet, world, .. } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let world = WorldContractReader::new(world_address, &provider)
                .with_block(BlockId::Tag(BlockTag::Pending));

            let model = world.model_reader(&name).await?;
            let entity = model.entity(&keys).await?;

            println!("{entity}")
        }
    }

    Ok(())
}
