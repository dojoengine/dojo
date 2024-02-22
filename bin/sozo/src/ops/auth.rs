use anyhow::{Context, Result};
use dojo_world::contracts::cairo_utils;
use dojo_world::contracts::world::WorldContract;
use dojo_world::metadata::Environment;
use dojo_world::utils::TransactionWaiter;
use starknet::accounts::Account;
use starknet::core::types::FieldElement;

use crate::commands::auth::AuthCommand;

pub async fn execute(command: AuthCommand, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        AuthCommand::Writer { models_contracts, world, starknet, account, transaction } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(&provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

            let mut calls = vec![];

            for mc in models_contracts {
                let parts: Vec<&str> = mc.split(',').collect();

                let (model, contract_part) = match parts.as_slice() {
                    [model, contract] => (model.to_string(), *contract),
                    _ => anyhow::bail!(
                        "Model and contract address are expected to be comma separated: `sozo \
                         auth writer model_name,0x1234`"
                    ),
                };

                let contract = FieldElement::from_hex_be(contract_part)
                    .map_err(|_| anyhow::anyhow!("Invalid contract address: {}", contract_part))?;

                calls.push(
                    world
                        .grant_writer_getcall(&cairo_utils::str_to_felt(&model)?, &contract.into()),
                );
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
    }

    Ok(())
}
