use std::sync::Arc;
use std::{fs, io};

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use dojo_world::contracts::WorldContract;
use dojo_world::manifest::{
    DojoContract, Manifest, OverlayClass, OverlayDojoContract, OverlayManifest, BASE_CONTRACT_NAME,
    BASE_DIR, CONTRACTS_DIR, MANIFESTS_DIR, OVERLAYS_DIR, WORLD_CONTRACT_NAME,
};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{DeployOutput, TxnConfig, UpgradeOutput};
use scarb::core::Workspace;
use starknet::accounts::{ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::FieldElement;
use starknet::providers::Provider;
use starknet::signers::Signer;

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
    pub name: String,
    pub contract_address: FieldElement,
    pub base_class_hash: FieldElement,
}

#[allow(clippy::too_many_arguments)]
pub async fn migrate<P, S>(
    ws: &Workspace<'_>,
    world_address: Option<FieldElement>,
    rpc_url: String,
    account: SingleOwnerAccount<P, S>,
    name: &str,
    dry_run: bool,
    txn_config: TxnConfig,
) -> Result<()>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();

    // its path to a file so `parent` should never return `None`
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();

    let profile_name =
        ws.current_profile().expect("Scarb profile expected to be defined.").to_string();
    let profile_dir = manifest_dir.join(MANIFESTS_DIR).join(&profile_name);

    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(ws.config().profile().as_str());

    // Load local and remote World manifests.
    let (local_manifest, remote_manifest) =
        utils::load_world_manifests(&profile_dir, &account, world_address, &ui).await.map_err(
            |e| {
                ui.error(e.to_string());
                anyhow!(
                    "\n Use `sozo clean` to clean your project.\nThen, rebuild your project with \
                     `sozo build`.",
                )
            },
        )?;

    // Calculate diff between local and remote World manifests.
    ui.print_step(2, "🧰", "Evaluating Worlds diff...");
    let mut diff = WorldDiff::compute(local_manifest.clone(), remote_manifest.clone());
    diff.update_order()?;

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

        let account = Arc::new(account);
        let world = WorldContract::new(world_address, account.clone());
        if let Some(migration_output) = migration_output {
            match auto_authorize(ws, &world, &txn_config, &local_manifest, &migration_output).await
            {
                Ok(()) => {
                    ui.print_sub("Auto authorize completed successfully");
                }
                Err(e) => {
                    ui.print_sub(format!("Failed to auto authorize with error: {e}"));
                }
            };

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
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let profile_dir = manifest_dir.join(MANIFESTS_DIR).join(profile_name);

    let base_manifests = profile_dir.join(BASE_DIR);

    let world = OverlayClass { name: WORLD_CONTRACT_NAME.into(), original_class_hash: None };
    let base = OverlayClass { name: BASE_CONTRACT_NAME.into(), original_class_hash: None };

    let contracts = overlay_dojo_contracts_from_path(&base_manifests.join(CONTRACTS_DIR))
        .with_context(|| "Failed to build default DojoContract Overlays from path.")?;
    // let models = overlay_model_from_path(&base_manifests.join(MODELS_DIR))?;

    let default_overlay = OverlayManifest { world: Some(world), base: Some(base), contracts };

    let overlay_path = profile_dir.join(OVERLAYS_DIR);
    let mut overlay_manifest = OverlayManifest::load_from_path(&overlay_path)
        .with_context(|| "Failed to load OverlayManifest from path.")?;
    overlay_manifest.merge(default_overlay);

    overlay_manifest
        .write_to_path_nested(&overlay_path)
        .with_context(|| "Failed to write OverlayManifest to path.")?;
    Ok(())
}

fn overlay_dojo_contracts_from_path(path: &Utf8PathBuf) -> Result<Vec<OverlayDojoContract>> {
    let mut elements = vec![];

    let entries = path
        .read_dir()?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    for path in entries {
        if path.is_file() {
            let manifest: Manifest<DojoContract> = toml::from_str(&fs::read_to_string(path)?)?;

            let overlay_manifest = OverlayDojoContract {
                name: manifest.name,
                original_class_hash: None,
                reads: None,
                writes: None,
                init_calldata: None,
            };
            elements.push(overlay_manifest);
        } else {
            continue;
        }
    }

    Ok(elements)
}

// fn overlay_model_from_path(path: &Utf8PathBuf) -> Result<Vec<OverlayDojoModel>> {
//     let mut elements = vec![];

//     let mut entries = path
//         .read_dir()?
//         .map(|entry| entry.map(|e| e.path()))
//         .collect::<Result<Vec<_>, io::Error>>()?;

//     // `read_dir` doesn't guarantee any order, so we sort the entries ourself.
//     // see: https://doc.rust-lang.org/std/fs/fn.read_dir.html#platform-specific-behavior
//     entries.sort();

//     for path in entries {
//         if path.is_file() {
//             let manifest: Manifest<DojoContract> = toml::from_str(&fs::read_to_string(path)?)?;

//             let overlay_manifest =
//                 OverlayDojoModel { name: manifest.name, original_class_hash: None };
//             elements.push(overlay_manifest);
//         } else {
//             continue;
//         }
//     }

//     Ok(elements)
// }
