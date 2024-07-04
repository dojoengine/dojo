use std::fs;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use dojo_world::contracts::WorldContract;
use dojo_world::manifest::utils::get_default_namespace_from_ws;
use dojo_world::manifest::{
    BaseManifest, OverlayClass, OverlayDojoContract, OverlayDojoModel, OverlayManifest,
    BASE_CONTRACT_TAG, BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR, WORLD_CONTRACT_TAG,
};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{DeployOutput, TxnConfig, UpgradeOutput};
use scarb::core::Workspace;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::FieldElement;

mod auto_auth;
mod migrate;
mod ui;
mod utils;

pub use self::auto_auth::auto_authorize;
use self::migrate::update_manifests_and_abis;
pub use self::migrate::{
    apply_diff, execute_strategy, prepare_migration, print_strategy, upload_metadata,
};
use self::ui::MigrationUi;

#[derive(Debug, Default, Clone)]
pub struct MigrationOutput {
    pub world_address: FieldElement,
    pub world_tx_hash: Option<FieldElement>,
    pub world_block_number: Option<u64>,
    // Represents if full migration got completeled.
    // If false that means migration got partially completed.
    pub full: bool,

    pub models: Vec<String>,
    pub contracts: Vec<Option<ContractMigrationOutput>>,
}

#[derive(Debug, Default, Clone)]
pub struct ContractMigrationOutput {
    pub tag: String,
    pub contract_address: FieldElement,
    pub base_class_hash: FieldElement,
}

#[allow(clippy::too_many_arguments)]
pub async fn migrate<A>(
    ws: &Workspace<'_>,
    world_address: Option<FieldElement>,
    rpc_url: String,
    account: A,
    name: &str,
    dry_run: bool,
    txn_config: TxnConfig,
    skip_manifests: Option<Vec<String>>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send,
    A::Provider: Send,
    A::SignError: 'static,
{
    let ui = ws.config().ui();

    // its path to a file so `parent` should never return `None`
    let root_dir = ws.manifest_path().parent().unwrap().to_path_buf();

    let profile_name =
        ws.current_profile().expect("Scarb profile expected to be defined.").to_string();
    let manifest_dir = root_dir.join(MANIFESTS_DIR).join(&profile_name);
    let manifest_base_dir = manifest_dir.join(BASE_DIR);
    let overlay_dir = root_dir.join(OVERLAYS_DIR).join(&profile_name);

    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(ws.config().profile().as_str());

    let default_namespace = get_default_namespace_from_ws(ws);

    // Load local and remote World manifests.
    let (local_manifest, remote_manifest) = utils::load_world_manifests(
        &manifest_base_dir,
        &overlay_dir,
        &account,
        world_address,
        &ui,
        skip_manifests,
    )
    .await
    .map_err(|e| {
        ui.error(e.to_string());
        anyhow!(
            "\n Use `sozo clean` to clean your project.\nThen, rebuild your project with `sozo \
             build`.",
        )
    })?;

    // Calculate diff between local and remote World manifests.
    ui.print_step(2, "🧰", "Evaluating Worlds diff...");
    let mut diff = WorldDiff::compute(local_manifest.clone(), remote_manifest.clone());
    diff.update_order(&default_namespace)?;

    let total_diffs = diff.count_diffs();
    ui.print_sub(format!("Total diffs found: {total_diffs}"));

    if total_diffs == 0 {
        ui.print("\n✨ No changes to be made. Remote World is already up to date!");
        return Ok(());
    }

    let mut strategy = prepare_migration(&target_dir, diff, name, world_address, &ui)?;
    let world_address = strategy.world_address().expect("world address must exist");
    strategy.resolve_variable(world_address)?;

    if dry_run {
        print_strategy(&ui, account.provider(), &strategy, world_address).await;

        update_manifests_and_abis(
            ws,
            local_manifest,
            &manifest_dir,
            &profile_name,
            &rpc_url,
            world_address,
            None,
            name,
        )
        .await?;
    } else {
        // Migrate according to the diff.
        let migration_output = match apply_diff(ws, &account, txn_config, &mut strategy).await {
            Ok(migration_output) => Some(migration_output),
            Err(e) => {
                update_manifests_and_abis(
                    ws,
                    local_manifest,
                    &manifest_dir,
                    &profile_name,
                    &rpc_url,
                    world_address,
                    None,
                    name,
                )
                .await?;
                return Err(e)?;
            }
        };

        update_manifests_and_abis(
            ws,
            local_manifest.clone(),
            &manifest_dir,
            &profile_name,
            &rpc_url,
            world_address,
            migration_output.clone(),
            name,
        )
        .await?;

        // TODO: temporary deactivate auto-auth because it should be adapted
        // with the new namespace feature.
        let account = Arc::new(account);
        let world = WorldContract::new(world_address, account.clone());
        if let Some(migration_output) = migration_output {
            // TODO
            if false {
                match auto_authorize(ws, &world, &txn_config, &local_manifest, &migration_output)
                    .await
                {
                    Ok(()) => {
                        ui.print_sub("Auto authorize completed successfully");
                    }
                    Err(e) => {
                        ui.print_sub(format!("Failed to auto authorize with error: {e}"));
                    }
                };
            }

            //
            if !ws.config().offline() {
                upload_metadata(ws, &account, migration_output.clone(), txn_config).await?;
            }
        }
    };

    Ok(())
}

enum ContractDeploymentOutput {
    AlreadyDeployed(FieldElement),
    Output(DeployOutput),
}

enum ContractUpgradeOutput {
    Output(UpgradeOutput),
}

pub fn generate_overlays(ws: &Workspace<'_>) -> Result<()> {
    let profile_name =
        ws.current_profile().expect("Scarb profile expected to be defined.").to_string();

    // its path to a file so `parent` should never return `None`
    let root_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let manifest_base_dir = root_dir.join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR);
    let overlay_dir = root_dir.join(OVERLAYS_DIR).join(&profile_name);

    let base_manifest = BaseManifest::load_from_path(&manifest_base_dir)?;

    let default_overlay = OverlayManifest {
        world: Some(OverlayClass {
            tag: WORLD_CONTRACT_TAG.to_string(),
            original_class_hash: None,
        }),
        base: Some(OverlayClass { tag: BASE_CONTRACT_TAG.to_string(), original_class_hash: None }),
        contracts: base_manifest
            .contracts
            .iter()
            .map(|c| OverlayDojoContract { tag: c.inner.tag.clone(), ..Default::default() })
            .collect::<Vec<_>>(),
        models: base_manifest
            .models
            .iter()
            .map(|m| OverlayDojoModel { tag: m.inner.tag.clone(), ..Default::default() })
            .collect::<Vec<_>>(),
    };

    if overlay_dir.exists() {
        // read existing OverlayManifest from path
        let mut overlay_manifest = OverlayManifest::load_from_path(&overlay_dir, &base_manifest)
            .with_context(|| "Failed to load OverlayManifest from path.")?;

        // merge them to get OverlayManifest which contains all the contracts and models from base
        // manifests
        overlay_manifest.merge(default_overlay);

        // to avoid duplicated overlay manifests, existing overlays must be removed before being
        // rewritten by `overlay_manifest.write_to_path_nested()`
        fs::remove_dir_all(&overlay_dir)?;
        fs::create_dir_all(&overlay_dir)?;

        overlay_manifest
            .write_to_path_nested(&overlay_dir)
            .with_context(|| "Failed to write OverlayManifest to path.")?;
    } else {
        fs::create_dir_all(&overlay_dir)?;
        default_overlay
            .write_to_path_nested(&overlay_dir)
            .with_context(|| "Failed to write OverlayManifest to path.")?;
    }

    Ok(())
}
