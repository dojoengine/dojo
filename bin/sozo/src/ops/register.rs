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
            let mut models_to_register = Vec::new();
            for model_class_hash in models.iter() {
                let (class_hash, _contract_address) = world.model(model_class_hash).call().await?;
                if class_hash.0 == FieldElement::ZERO {
                    models_to_register.push(*model_class_hash);
                } else {
                    config.ui().print(format!(
                        "\"{:#x}\" model already registered with the given class hash",
                        model_class_hash
                    ));
                }
            }

            if models_to_register.is_empty() {
                config.ui().print("No new models to register.");
                return Ok(());
            }

            let calls = models_to_register
                .iter()
                .map(|c| world.register_model_getcall(&(*c).into()))
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
