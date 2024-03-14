use std::collections::HashMap;

use anyhow::{Context, Result};
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::manifest::DeployedManifest;
use dojo_world::metadata::Environment;
use scarb::core::Config;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};

use crate::commands::register::RegisterCommand;
use crate::utils::handle_transaction_result;

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
            let manifest = {
                match DeployedManifest::load_from_remote(&provider, world_address).await {
                    Ok(manifest) => manifest,
                    Err(e) => {
                        return Err(anyhow::anyhow!("Failed to build remote World state: {e}"));
                    }
                }
            };

            let registered_models_names = manifest.models.iter().map(|m| m.name.as_str());
            let mut model_class_hashes = HashMap::new();
            for model_name in registered_models_names {
                let read_model = world_reader.model_reader(model_name).await?;
                let class_hash = read_model.class_hash();
                model_class_hashes.insert(class_hash, model_name);
            }

            let mut models_to_register = Vec::new();
            for input_model in models {
                if let Some(model_name) = model_class_hashes.get(&input_model) {
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

            handle_transaction_result(&provider, res, transaction.wait, transaction.receipt)
                .await?;
        }
    }
    Ok(())
}
