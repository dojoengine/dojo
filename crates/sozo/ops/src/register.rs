use std::collections::HashMap;

use anyhow::Result;
use dojo_utils::TxnConfig;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::manifest::DeploymentManifest;
use scarb::core::Config;
#[cfg(feature = "walnut")]
use sozo_walnut::WalnutDebugger;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::Felt;
use starknet::providers::Provider;

use crate::utils::handle_transaction_result;

pub async fn model_register<A, P>(
    models: Vec<Felt>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    world_reader: WorldContractReader<P>,
    world_address: Felt,
    config: &Config,
    #[cfg(feature = "walnut")] walnut_debugger: &Option<WalnutDebugger>,
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

    let registered_models = manifest.models.iter().map(|m| m.inner.tag.clone());
    let mut model_class_hashes = HashMap::new();
    for model_tag in registered_models {
        let read_model = world_reader.model_reader_with_tag(&model_tag).await?;
        let class_hash = read_model.class_hash();
        model_class_hashes.insert(class_hash, model_tag);
    }

    let mut models_to_register = Vec::new();
    for input_model in models {
        if let Some(model_tag) = model_class_hashes.get(&input_model) {
            config.ui().print(format!(
                "\"{}\" model already registered with the class hash \"{:#x}\"",
                model_tag, input_model
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

    let Some(invoke_res) =
        dojo_utils::handle_execute(txn_config.fee_setting, &world.account, calls).await?
    else {
        todo!("handle estimate and simulate")
    };

    handle_transaction_result(
        &config.ui(),
        &world.account.provider(),
        invoke_res,
        txn_config.wait,
        txn_config.receipt,
        #[cfg(feature = "walnut")]
        walnut_debugger,
    )
    .await?;

    Ok(())
}
