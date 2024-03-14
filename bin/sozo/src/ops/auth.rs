use anyhow::{Context, Result};
use dojo_world::contracts::model::ModelError;
use dojo_world::contracts::world::WorldContract;
use dojo_world::contracts::WorldContractReader;
use dojo_world::metadata::Environment;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::parse_cairo_short_string;

use super::get_contract_address;
use crate::commands::auth::{AuthCommand, AuthKind, ResourceType};
use crate::utils::handle_transaction_result;

pub async fn execute(command: AuthCommand, env_metadata: Option<Environment>) -> Result<()> {
    match command {
        AuthCommand::Grant { kind, world, starknet, account, transaction } => match kind {
            AuthKind::Writer { models_contracts } => {
                let world_address = world.address(env_metadata.as_ref())?;
                let provider = starknet.provider(env_metadata.as_ref())?;

                let account = account.account(&provider, env_metadata.as_ref()).await?;
                let world = WorldContract::new(world_address, &account);
                let world_reader = WorldContractReader::new(world_address, &provider)
                    .with_block(BlockId::Tag(BlockTag::Pending));

                let mut calls = Vec::new();

                for mc in models_contracts {
                    let model_name = parse_cairo_short_string(&mc.model)?;

                    match world_reader.model_reader(&model_name).await {
                        Ok(_) => {
                            let contract = get_contract_address(&world, mc.contract).await?;
                            calls.push(world.grant_writer_getcall(&mc.model, &contract.into()));
                        }

                        Err(ModelError::ModelNotFound) => {
                            println!("Unknown model '{}' => IGNORED", model_name);
                        }

                        Err(err) => {
                            return Err(err.into());
                        }
                    }
                }

                if !calls.is_empty() {
                    let res = account
                        .execute(calls)
                        .send()
                        .await
                        .with_context(|| "Failed to send transaction")?;

                    handle_transaction_result(
                        &provider,
                        res,
                        transaction.wait,
                        transaction.receipt,
                    )
                    .await?;
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

                handle_transaction_result(&provider, res, transaction.wait, transaction.receipt)
                    .await?;
            }
        },
        _ => todo!(),
    }

    Ok(())
}
