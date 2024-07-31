use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use dojo_world::contracts::WorldContract;
use dojo_world::manifest::{BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR};
use dojo_world::metadata::get_default_namespace_from_ws;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{DeployOutput, TxnConfig, UpgradeOutput};
use scarb::core::Workspace;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::Felt;
use starknet::core::utils::{cairo_short_string_to_felt, get_contract_address};
use starknet_crypto::poseidon_hash_single;

mod auto_auth;
mod migrate;
mod ui;
mod utils;

pub use self::auto_auth::auto_authorize;
pub use self::migrate::{
    apply_diff, execute_strategy, prepare_migration, print_strategy, upload_metadata,
};
use self::migrate::{find_authorization_diff, update_manifests_and_abis};
use self::ui::MigrationUi;

#[derive(Debug, Default, Clone)]
pub struct MigrationOutput {
    pub world_address: Felt,
    pub world_tx_hash: Option<Felt>,
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
    pub contract_address: Felt,
    pub base_class_hash: Felt,
}

#[allow(clippy::too_many_arguments)]
pub async fn migrate<A>(
    ws: &Workspace<'_>,
    world_address: Option<Felt>,
    rpc_url: String,
    account: A,
    name: &str,
    dry_run: bool,
    txn_config: TxnConfig,
    skip_manifests: Option<Vec<String>>,
) -> Result<Option<MigrationOutput>>
where
    A: ConnectedAccount + Sync + Send + 'static,
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

    let default_namespace = get_default_namespace_from_ws(ws)?;

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

    let generated_world_address = get_world_address(&local_manifest, name)?;
    if let Some(world_address) = world_address {
        if world_address != generated_world_address {
            bail!(format!(
                "Calculated world address ({:#x}) doesn't match provided world address. If you \
                 are deploying with custom seed make sure `world_address` is correctly configured \
                 (or not set) `Scarb.toml`",
                generated_world_address
            ))
        }
    }

    // Calculate diff between local and remote World manifests.
    ui.print_step(2, "🧰", "Evaluating Worlds diff...");
    let diff =
        WorldDiff::compute(local_manifest.clone(), remote_manifest.clone(), &default_namespace)?;

    let total_diffs = diff.count_diffs();
    ui.print_sub(format!("Total diffs found: {total_diffs}"));

    if total_diffs == 0 {
        ui.print("\n✨ No diffs found. Remote World is already up to date!");
    }

    let strategy = prepare_migration(&target_dir, diff.clone(), name, world_address, &ui)?;
    // TODO: dry run can also show the diffs for things apart from world state
    // what new authorizations would be granted, if ipfs data would change or not,
    // etc...
    if dry_run {
        if total_diffs == 0 {
            return Ok(None);
        }

        print_strategy(&ui, account.provider(), &strategy, strategy.world_address).await;

        update_manifests_and_abis(
            ws,
            local_manifest,
            &manifest_dir,
            &profile_name,
            &rpc_url,
            strategy.world_address,
            None,
            name,
        )
        .await?;

        Ok(None)
    } else {
        let migration_output = if total_diffs != 0 {
            match apply_diff(ws, &account, txn_config, &strategy).await {
                Ok(migration_output) => Some(migration_output),
                Err(e) => {
                    update_manifests_and_abis(
                        ws,
                        local_manifest,
                        &manifest_dir,
                        &profile_name,
                        &rpc_url,
                        strategy.world_address,
                        None,
                        name,
                    )
                    .await?;
                    return Err(e)?;
                }
            }
        } else {
            None
        };

        update_manifests_and_abis(
            ws,
            local_manifest.clone(),
            &manifest_dir,
            &profile_name,
            &rpc_url,
            strategy.world_address,
            migration_output.clone(),
            name,
        )
        .await?;

        let account = Arc::new(account);
        let world = WorldContract::new(strategy.world_address, account.clone());

        ui.print(" ");
        ui.print_step(6, "🖋️", "Authorizing systems based on overlay...");
        let (grant, revoke) = find_authorization_diff(
            &ui,
            &world,
            &diff,
            migration_output.as_ref(),
            &default_namespace,
        )
        .await?;

        match auto_authorize(ws, &world, &txn_config, &default_namespace, &grant, &revoke).await {
            Ok(()) => {
                ui.print_sub("Auto authorize completed successfully");
            }
            Err(e) => {
                ui.print_sub(format!("Failed to auto authorize with error: {e}"));
            }
        };

        if let Some(migration_output) = &migration_output {
            if !ws.config().offline() {
                upload_metadata(ws, &account, migration_output.clone(), txn_config).await?;
            }
        }

        Ok(migration_output)
    }
}

fn get_world_address(
    local_manifest: &dojo_world::manifest::BaseManifest,
    name: &str,
) -> Result<Felt> {
    let name = cairo_short_string_to_felt(name)?;
    let salt = poseidon_hash_single(name);

    let generated_world_address = get_contract_address(
        salt,
        local_manifest.world.inner.original_class_hash,
        &[local_manifest.base.inner.class_hash],
        Felt::ZERO,
    );

    Ok(generated_world_address)
}

#[allow(dead_code)]
enum ContractDeploymentOutput {
    AlreadyDeployed(Felt),
    Output(DeployOutput),
}

#[allow(dead_code)]
enum ContractUpgradeOutput {
    Output(UpgradeOutput),
}
