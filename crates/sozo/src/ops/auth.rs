use anyhow::{Context, Result};
use dojo_world::contracts::cairo_utils;
use dojo_world::contracts::world::WorldContract;
use dojo_world::metadata::Environment;

use crate::commands::auth::AuthCommand;

pub async fn execute(command: AuthCommand, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        AuthCommand::Writer { model, contract, world, starknet, account } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

            let res = world
                .grant_writer(&cairo_utils::str_to_felt(&model)?, &contract.into())
                .send()
                .await
                .with_context(|| "Failed to send transaction")?;

            println!("Transaction: {:#x}", res.transaction_hash);
        }
    }

    Ok(())
}
