use anyhow::{Context, Result};
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

            let world_reader = WorldContractReader::new(world_address, &provider)
                .with_block(BlockId::Tag(BlockTag::Pending));
            let remote_manifest = {
                match Manifest::load_from_remote(&provider, world_address).await {
                    Ok(manifest) => Some(manifest),
                    Err(ManifestError::RemoteWorldNotFound) => None,
                    Err(e) => {
                        return Err(anyhow!("Failed to build remote World state: {e}"));
                    }
                }
            };

            let manifest = remote_manifest.unwrap();
            let registered_models_names =
                manifest.models.into_iter().map(|m| m.name).collect::<Vec<String>>();

            let mut model_class_hashes = HashMap::new();
            for model_name in registered_models_names.iter() {
                let read_model = world_reader.model_reader(model_name).await?;
                let class_hash = read_model.class_hash();
                model_class_hashes.insert(class_hash, model_name.clone());
            }

            let calls = models
                .iter()
                .filter(|m| {
                    if model_class_hashes.contains_key(m) {
                        let model_name = model_class_hashes.get(&m).unwrap();
                        config.ui().print(format!("\"{model_name}\" model already registered with the given class hash"));
                        return false;
                    }
                    true
                })
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
