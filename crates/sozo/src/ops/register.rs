use anyhow::{Context, Result};
use dojo_world::contracts::cairo_utils::str_to_felts;
use dojo_world::contracts::WorldContract;
use dojo_world::metadata::Environment;
use dojo_world::utils::TransactionWaiter;
use starknet::accounts::Account;

use crate::commands::register::RegisterCommand;

pub async fn execute(command: RegisterCommand, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        RegisterCommand::Model { models, world, starknet, account, transaction } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(&provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

            let calls = models
                .iter()
                .map(|m| {
                    world.register_model_getcall(
                        &str_to_felts(&m.name).unwrap(),
                        &m.class_hash.into(),
                    )
                })
                .collect::<Vec<_>>();

            let res = account
                .execute(calls)
                .send()
                .await
                .with_context(|| "Failed to send transaction")?;

            if transaction.wait {
                let receipt = TransactionWaiter::new(res.transaction_hash, &provider).await?;
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            } else {
                println!("Transaction hash: {:#x}", res.transaction_hash);
            }
        }
    }
    Ok(())
}
