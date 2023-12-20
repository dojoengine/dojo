use anyhow::{Context, Result};
use dojo_world::contracts::WorldContract;
use dojo_world::metadata::Environment;
use starknet::accounts::Account;

use crate::commands::register::RegisterCommand;

pub async fn execute(command: RegisterCommand, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        RegisterCommand::Model { models, world, starknet, account } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

            let calls = models
                .iter()
                .map(|c| world.register_model_getcall(&(*c).into()))
                .collect::<Vec<_>>();

            let res = account
                .execute(calls)
                .send()
                .await
                .with_context(|| "Failed to send transaction")?;

            println!("Models registered at transaction: {:#x}", res.transaction_hash)
        }
    }
    Ok(())
}
