use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use dojo_lang::compiler::{ABIS_DIR, BASE_DIR, DEPLOYMENTS_DIR, MANIFESTS_DIR};
use dojo_world::manifest::{
    AbiFormat, BaseManifest, DeploymentManifest, DojoContract, DojoModel, Manifest,
    ManifestMethods, WorldContract as ManifestWorldContract, WorldMetadata,
};
use dojo_world::migration::strategy::generate_salt;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{DeployOutput, TxnConfig, UpgradeOutput};
use scarb::core::Workspace;
use starknet::accounts::{ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_contract_address;
use starknet::providers::Provider;
use starknet::signers::Signer;
use tokio::fs;

mod migrate;
mod ui;
mod utils;

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
    account: &SingleOwnerAccount<P, S>,
    name: Option<String>,
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
        utils::load_world_manifests(&profile_dir, account, world_address, &ui).await.map_err(
            |e| {
                ui.error(e.to_string());
                anyhow!(
                    "\n Use `sozo clean` to clean your project, or `sozo clean --manifests-abis` \
                     to clean manifest and abi files only.\nThen, rebuild your project with `sozo \
                     build`.",
                )
            },
        )?;

    // Calculate diff between local and remote World manifests.
    ui.print_step(2, "🧰", "Evaluating Worlds diff...");
    let diff = WorldDiff::compute(local_manifest.clone(), remote_manifest.clone());
    let total_diffs = diff.count_diffs();
    ui.print_sub(format!("Total diffs found: {total_diffs}"));

    if total_diffs == 0 {
        ui.print("\n✨ No changes to be made. Remote World is already up to date!");
        return Ok(());
    }

    let mut strategy = prepare_migration(&target_dir, diff, name.clone(), world_address, &ui)?;
    let world_address = strategy.world_address().expect("world address must exist");

    if dry_run {
        print_strategy(&ui, account.provider(), &strategy).await;

        update_manifests_and_abis(
            ws,
            local_manifest,
            &profile_dir,
            &profile_name,
            &rpc_url,
            world_address,
            None,
            name.as_ref(),
        )
        .await?;
    } else {
        // Migrate according to the diff.
        let migration_output = match apply_diff(ws, account, txn_config, &mut strategy).await {
            Ok(migration_output) => Some(migration_output),
            Err(e) => {
                update_manifests_and_abis(
                    ws,
                    local_manifest,
                    &profile_dir,
                    &profile_name,
                    &rpc_url,
                    world_address,
                    None,
                    name.as_ref(),
                )
                .await?;
                return Err(e)?;
            }
        };

        update_manifests_and_abis(
            ws,
            local_manifest.clone(),
            &profile_dir,
            &profile_name,
            &rpc_url,
            world_address,
            migration_output.clone(),
            name.as_ref(),
        )
        .await?;

        if let Some(migration_output) = migration_output {
            if !ws.config().offline() {
                upload_metadata(ws, account, migration_output, txn_config).await?;
            }
        }
    };

    Ok(())
}

async fn update_manifests_and_abis(
    ws: &Workspace<'_>,
    local_manifest: BaseManifest,
    profile_dir: &Utf8PathBuf,
    profile_name: &str,
    rpc_url: &str,
    world_address: FieldElement,
    migration_output: Option<MigrationOutput>,
    salt: Option<&String>,
) -> Result<()> {
    let ui = ws.config().ui();
    ui.print_step(5, "✨", "Updating manifests...");

    let deployed_path = profile_dir.join("manifest").with_extension("toml");
    let deployed_path_json = profile_dir.join("manifest").with_extension("json");

    let mut local_manifest: DeploymentManifest = local_manifest.into();

    local_manifest.world.inner.metadata = Some(WorldMetadata {
        profile_name: profile_name.to_string(),
        rpc_url: rpc_url.to_string(),
    });

    if deployed_path.exists() {
        let previous_manifest = DeploymentManifest::load_from_path(&deployed_path)?;
        local_manifest.merge_from_previous(previous_manifest);
    };

    local_manifest.world.inner.address = Some(world_address);
    if let Some(salt) = salt {
        local_manifest.world.inner.seed = Some(salt.to_owned());
    }

    // when the migration has not been applied because in `plan` mode or because of an error,
    // the `migration_output` is empty.
    if let Some(migration_output) = migration_output {
        if migration_output.world_tx_hash.is_some() {
            local_manifest.world.inner.transaction_hash = migration_output.world_tx_hash;
        }
        if migration_output.world_block_number.is_some() {
            local_manifest.world.inner.block_number = migration_output.world_block_number;
        }

        migration_output.contracts.iter().for_each(|contract_output| {
            // ignore failed migration which are represented by None
            if let Some(output) = contract_output {
                // find the contract in local manifest and update its address and base class hash
                let local = local_manifest
                    .contracts
                    .iter_mut()
                    .find(|c| c.name == output.name)
                    .expect("contract got migrated, means it should be present here");

                local.inner.base_class_hash = output.base_class_hash;
            }
        });
    }

    local_manifest.contracts.iter_mut().for_each(|contract| {
        let salt = generate_salt(&contract.name);
        contract.inner.address =
            Some(get_contract_address(salt, contract.inner.base_class_hash, &[], world_address));
    });

    // copy abi files from `abi/base` to `abi/deployments/{chain_id}` and update abi path in
    // local_manifest
    update_manifest_abis(&mut local_manifest, profile_dir, profile_name).await;

    local_manifest.write_to_path_toml(&deployed_path)?;
    local_manifest.write_to_path_json(&deployed_path_json, profile_dir)?;
    ui.print("\n✨ Done.");

    Ok(())
}

async fn update_manifest_abis(
    local_manifest: &mut DeploymentManifest,
    profile_dir: &Utf8PathBuf,
    profile_name: &str,
) {
    fs::create_dir_all(profile_dir.join(ABIS_DIR).join(DEPLOYMENTS_DIR))
        .await
        .expect("Failed to create folder");

    async fn inner_helper<T>(
        profile_dir: &Utf8PathBuf,
        profile_name: &str,
        manifest: &mut Manifest<T>,
    ) where
        T: ManifestMethods,
    {
        // Unwraps in call to abi is safe because we always write abis for DojoContracts as relative
        // path.
        // In this relative path, we only what the root from
        // ABI directory.
        let base_relative_path = manifest
            .inner
            .abi()
            .unwrap()
            .to_path()
            .unwrap()
            .strip_prefix(Utf8PathBuf::new().join(MANIFESTS_DIR).join(profile_name))
            .unwrap();

        // The filename is safe to unwrap as it's always
        // present in the base relative path.
        let deployed_relative_path = Utf8PathBuf::new().join(ABIS_DIR).join(DEPLOYMENTS_DIR).join(
            base_relative_path
                .strip_prefix(Utf8PathBuf::new().join(ABIS_DIR).join(BASE_DIR))
                .unwrap(),
        );

        let full_base_path = profile_dir.join(base_relative_path);
        let full_deployed_path = profile_dir.join(deployed_relative_path.clone());

        fs::create_dir_all(full_deployed_path.parent().unwrap())
            .await
            .expect("Failed to create folder");

        fs::copy(full_base_path, full_deployed_path).await.expect("Failed to copy abi file");

        manifest.inner.set_abi(Some(AbiFormat::Path(deployed_relative_path)));
    }

    inner_helper::<ManifestWorldContract>(profile_dir, profile_name, &mut local_manifest.world)
        .await;

    for contract in local_manifest.contracts.iter_mut() {
        inner_helper::<DojoContract>(profile_dir, profile_name, contract).await;
    }

    for model in local_manifest.models.iter_mut() {
        inner_helper::<DojoModel>(profile_dir, profile_name, model).await;
    }
}

enum ContractDeploymentOutput {
    AlreadyDeployed(FieldElement),
    Output(DeployOutput),
}

enum ContractUpgradeOutput {
    Output(UpgradeOutput),
}
