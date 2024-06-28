use anyhow::Result;
use dojo_world::contracts::{cairo_utils, WorldContract};
use dojo_world::manifest::BaseManifest;
use dojo_world::migration::TxnConfig;
use scarb::core::Workspace;
use scarb_ui::Ui;
use starknet::accounts::ConnectedAccount;

use super::ui::MigrationUi;
use super::MigrationOutput;
use crate::auth::{grant_writer, ModelContract};

pub async fn auto_authorize<A>(
    ws: &Workspace<'_>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    local_manifest: &BaseManifest,
    migration_output: &MigrationOutput,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send,
    A::SignError: 'static,
{
    let ui = ws.config().ui();

    ui.print(" ");
    ui.print_step(6, "🖋️", "Authorizing Models to Systems (based on overlay)...");
    ui.print(" ");
    let models_contracts = compute_models_contracts(&ui, local_manifest, migration_output)?;
    grant_writer(&ui, world, models_contracts, *txn_config).await
}

pub fn compute_models_contracts(
    ui: &Ui,
    local_manifest: &BaseManifest,
    migration_output: &MigrationOutput,
) -> Result<Vec<crate::auth::ModelContract>> {
    let mut res = vec![];
    let local_contracts = &local_manifest.contracts;

    // From all the contracts that were migrated successfully.
    for migrated_contract in migration_output.contracts.iter().flatten() {
        // Find that contract from local_manifest based on its name.
        let contract = local_contracts
            .iter()
            .find(|c| migrated_contract.tag == c.inner.tag)
            .expect("we know this contract exists");

        ui.print_sub(format!(
            "Authorizing {} for Models: {:?}",
            contract.inner.tag, contract.inner.writes
        ));

        // Read all the models that its supposed to write and collect them in a Vec<ModelContract>
        // so we can call `grant_writer` on all of them.
        for model in &contract.inner.writes {
            let model = cairo_utils::str_to_felt(model)?;
            let contract_addr_str = format!("{:#x}", migrated_contract.contract_address);

            res.push(ModelContract { model, contract: contract_addr_str });
        }
    }

    Ok(res)
}
