use anyhow::{Context, Result};
use dojo_client::contract::world::WorldContract;
use toml::Value;

use crate::commands::auth::AuthCommand;

pub async fn execute(command: AuthCommand, env_metadata: Option<Value>) -> Result<()> {
    match command {
        AuthCommand::Writer { component, system, world, starknet, account } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

            let res = world
                .grant_writer(&component, &system)
                .await
                .with_context(|| "Failed to send transaction")?;

            println!("Transaction: {:#x}", res.transaction_hash);
        }
    }

    Ok(())
}
