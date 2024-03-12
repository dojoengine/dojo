use std::collections::HashMap;

use anyhow::{Context, Result};
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::manifest::DeployedManifest;
use dojo_world::metadata::Environment;
use dojo_world::utils::TransactionWaiter;
use scarb::core::Config;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};

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

            let world_reader = WorldContractReader::new(world_address, &provider)
                .with_block(BlockId::Tag(BlockTag::Pending));
            let remote_manifest = {
                match DeployedManifest::load_from_remote(&provider, world_address).await {
                    Ok(manifest) => Some(manifest),
                    Err(e) => {
                        return Err(anyhow::anyhow!("Failed to build remote World state: {e}"));
                    }
                }
            };

            let manifest = remote_manifest.unwrap();
            let registered_models_names =
                manifest.models.into_iter().map(|m| m.name.to_string()).collect::<Vec<String>>();

            let mut model_class_hashes = HashMap::new();
            for model_name in registered_models_names.iter() {
                let read_model = world_reader.model_reader(model_name).await?;
                let class_hash = read_model.class_hash();
                model_class_hashes.insert(class_hash, model_name.clone());
            }

            let mut models_to_register = Vec::new();
            for input_model in models {
                if model_class_hashes.contains_key(&input_model) {
                    let model_name = model_class_hashes.get(&input_model).unwrap();
                    config.ui().print(format!(
                        "\"{}\" model already registered with the class hash \"{:#x}\"",
                        model_name, input_model
                    ));
                } else {
                    models_to_register.push(input_model);
                }
            }

            if models_to_register.is_empty() {
                config.ui().print("No new models to register.");
                return Ok(());
            }

            let account = account.account(&provider, env_metadata.as_ref()).await?;
            let world = WorldContract::new(world_address, &account);

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
