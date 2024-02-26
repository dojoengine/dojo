use anyhow::{Context, Result};
use dojo_world::contracts::world::WorldContract;
use dojo_world::metadata::Environment;
use dojo_world::utils::TransactionWaiter;
use starknet::accounts::Account;

use super::get_contract_address;
use crate::commands::auth::{AuthCommand, AuthKind, ResourceType};

pub async fn execute(command: AuthCommand, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        AuthCommand::Grant { kind, world, starknet, account, transaction } => match kind {
            AuthKind::Writer { models_contracts } => {
                let world_address = world.address(env_metadata.as_ref())?;
                let provider = starknet.provider(env_metadata.as_ref())?;

                let account = account.account(&provider, env_metadata.as_ref()).await?;
                let world = WorldContract::new(world_address, &account);

                let mut calls = Vec::new();

                for mc in models_contracts {
                    let contract = get_contract_address(&world, mc.contract).await?;
                    calls.push(world.grant_writer_getcall(&mc.model, &contract.into()));
                }

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
            AuthKind::Owner { owners_resources } => {
                let world_address = world.address(env_metadata.as_ref())?;
                let provider = starknet.provider(env_metadata.as_ref())?;

                let account = account.account(&provider, env_metadata.as_ref()).await?;
                let world = WorldContract::new(world_address, &account);

                let mut calls = Vec::new();

                for or in owners_resources {
                    let resource = match &or.resource {
                        ResourceType::Model(name) => *name,
                        ResourceType::Contract(name_or_address) => {
                            get_contract_address(&world, name_or_address.clone()).await?
                        }
                    };

                    calls.push(world.grant_owner_getcall(&or.owner.into(), &resource));
                }

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
        },
        _ => todo!(),
    }

    Ok(())
}
