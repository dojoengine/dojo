use std::collections::HashMap;

use anyhow::{Context, Result};
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::manifest::DeploymentManifest;
use dojo_world::migration::TxnConfig;
use dojo_world::utils::{get_full_world_element_name, TransactionExt};
use scarb::core::Config;
use starknet::accounts::ConnectedAccount;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;

use crate::utils::handle_transaction_result;

pub async fn model_register<A, P>(
    models: Vec<FieldElement>,
    world: &WorldContract<A>,
    txn_config: TxnConfig,
    world_reader: WorldContractReader<P>,
    world_address: FieldElement,
    config: &Config,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
    P: Provider + Sync + Send,
{
    let manifest = {
        match DeploymentManifest::load_from_remote(&world.account.provider(), world_address).await {
            Ok(manifest) => manifest,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to build remote World state: {e}"));
            }
        }
    };

    let registered_models =
        manifest.models.iter().map(|m| (m.inner.namespace.clone(), m.inner.name.clone()));
    let mut model_class_hashes = HashMap::new();
    for (namespace, model_name) in registered_models {
        let read_model = world_reader.model_reader(&namespace, &model_name).await?;
        let class_hash = read_model.class_hash();
        model_class_hashes.insert(class_hash, (namespace, model_name));
    }

    let mut models_to_register = Vec::new();
    for input_model in models {
        if let Some((namespace, model_name)) = model_class_hashes.get(&input_model) {
            config.ui().print(format!(
                "\"{}\" model already registered with the class hash \"{:#x}\"",
                get_full_world_element_name(namespace, model_name),
                input_model
            ));
        } else {
            models_to_register.push(input_model);
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

    let res = world
        .account
        .execute(calls)
        .send_with_cfg(&txn_config)
        .await
        .with_context(|| "Failed to send transaction")?;

    handle_transaction_result(
        &config.ui(),
        &world.account.provider(),
        res,
        txn_config.wait,
        txn_config.receipt,
    )
    .await?;

    Ok(())
}
