use anyhow::{Context, Result};
use dojo_world::world::WorldContract;
use toml::Value;

use crate::commands::register::RegisterCommand;

pub async fn execute(command: RegisterCommand, env_metadata: Option<Value>) -> Result<()> {
    match command {
        RegisterCommand::Component { components, world, starknet, account } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

            let res = world
                .register_components(&components)
                .await
                .with_context(|| "Failed to send transaction")?;

            println!("Components registered at transaction: {:#x}", res.transaction_hash)
        }

        RegisterCommand::System { systems, world, starknet, account } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

            let res = world
                .register_systems(&systems)
                .await
                .with_context(|| "Failed to send transaction")?;

            println!("Systems registered at transaction: {:#x}", res.transaction_hash)
        }
    }
    Ok(())
}
