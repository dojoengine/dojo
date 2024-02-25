use std::collections::HashMap;

use anyhow::{Context, Result};
use dojo_world::contracts::WorldContract;
use dojo_world::metadata::Environment;
use dojo_world::utils::TransactionWaiter;
use scarb::core::Config;
use starknet::accounts::Account;
use starknet_crypto::FieldElement;

use crate::commands::register::RegisterCommand;

pub async fn execute(
    command: RegisterCommand,
    env_metadata: Option<Environment>,
    config: &Config,
) -> Result<()> {
    match command {
        RegisterCommand::Model { models, world, starknet, account, transaction } => {
            let world_address = world.address(env_metadata.as_ref())?;
            let provider = starknet.provider(env_metadata.as_ref())?;

            let account = account.account(&provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);
            let mut model_class_hashes = HashMap::new();
            for model_class_hash in models.iter() {
                let (class_hash, contract_address) =
                    world.model(model_class_hash.into()).call().await?;
                if class_hash.0 != FieldElement::from_hex_be("0x0")?
                    && contract_address.0 != FieldElement::from_hex_be("0x0")?
                {
                    model_class_hashes.insert(model_class_hash, true);
                }
            }

            }

            let calls = models
                .iter()
                .filter(|m| {
                    if model_class_hashes.contains_key(m) {
                        config.ui().print(format!(
                            "\"{:?}\" model already registered with the given class hash", m
                        ));
                        return false;
                    }
                    true
                })
                .map(|c| world.register_model_getcall(&(*c).into()))
                .collect::<Vec<_>>();

            if calls.len() == 0 {
                config.ui().print("No new models to register.");
                return Ok(());
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
