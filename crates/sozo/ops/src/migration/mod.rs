use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8PathBuf;
use dojo_lang::compiler::{ABIS_DIR, BASE_DIR, DEPLOYMENTS_DIR, MANIFESTS_DIR, OVERLAYS_DIR};
use dojo_world::contracts::abi::world::ResourceMetadata;
use dojo_world::contracts::cairo_utils;
use dojo_world::contracts::world::WorldContract;
use dojo_world::manifest::{
    AbiFormat, AbstractManifestError, BaseManifest, DeploymentManifest, DojoContract, DojoModel,
    Manifest, ManifestMethods, OverlayManifest, WorldContract as ManifestWorldContract,
    WorldMetadata,
};
use dojo_world::metadata::dojo_metadata_from_workspace;
use dojo_world::migration::contract::ContractMigration;
use dojo_world::migration::strategy::{generate_salt, prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{
    Declarable, DeployOutput, Deployable, MigrationError, RegisterOutput, StateDiff, TxConfig,
    Upgradable, UpgradeOutput,
};
use dojo_world::utils::TransactionWaiter;
use scarb::core::Workspace;
use scarb_ui::Ui;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, FieldElement, FunctionCall, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_contract_address, get_selector_from_name,
};
use starknet::providers::{Provider, ProviderError};
use tokio::fs;

#[cfg(test)]
#[path = "migration_test.rs"]
mod migration_test;
mod ui;

use starknet::signers::Signer;
use ui::MigrationUi;

use self::ui::{bold_message, italic_message};

#[derive(Debug, Default, Clone)]
pub struct MigrationOutput {
    pub world_address: FieldElement,
    pub world_tx_hash: Option<FieldElement>,
    pub world_block_number: Option<u64>,
    // Represents if full migration got completeled.
    // If false that means migration got partially completed.
    pub full: bool,

    pub contracts: Vec<Option<DeployOutput>>,
}

pub async fn migrate<P, S>(
    ws: &Workspace<'_>,
    world_address: Option<FieldElement>,
    chain_id: String,
    rpc_url: String,
    account: &SingleOwnerAccount<P, S>,
    name: Option<String>,
    dry_run: bool,
    txn_config: Option<TxConfig>,
) -> Result<()>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();

    // Setup account for migration and fetch world address if it exists.
    ui.print(format!("Chain ID: {}\n", &chain_id));

    // its path to a file so `parent` should never return `None`
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();

    let profile_name =
        ws.current_profile().expect("Scarb profile expected to be defined.").to_string();
    let profile_dir = manifest_dir.join(MANIFESTS_DIR).join(&profile_name);

    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(ws.config().profile().as_str());

    // Load local and remote World manifests.
    let (local_manifest, remote_manifest) =
        load_world_manifests(&profile_dir, account, world_address, &ui).await.map_err(|e| {
            ui.error(e.to_string());
            anyhow!(
                "\n Use `sozo clean` to clean your project, or `sozo clean --manifests-abis` to \
                 clean manifest and abi files only.\nThen, rebuild your project with `sozo build`.",
            )
        })?;

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

    let migration_output = if dry_run {
        print_strategy(&ui, account.provider(), &strategy).await;
        MigrationOutput { world_address, ..Default::default() }
    } else {
        // Migrate according to the diff.
        match apply_diff(ws, account, txn_config, &mut strategy).await {
            Ok(migration_output) => migration_output,
            Err(e) => {
                update_manifests_and_abis(
                    ws,
                    local_manifest,
                    &profile_dir,
                    &profile_name,
                    &rpc_url,
                    MigrationOutput { world_address, ..Default::default() },
                    name.as_ref(),
                )
                .await?;
                return Err(e)?;
            }
        }
    };

    update_manifests_and_abis(
        ws,
        local_manifest,
        &profile_dir,
        &profile_name,
        &rpc_url,
        migration_output,
        name.as_ref(),
    )
    .await?;

    Ok(())
}

async fn update_manifests_and_abis(
    ws: &Workspace<'_>,
    local_manifest: BaseManifest,
    profile_dir: &Utf8PathBuf,
    profile_name: &str,
    rpc_url: &str,
    migration_output: MigrationOutput,
    salt: Option<&String>,
) -> Result<()> {
    let ui = ws.config().ui();
    ui.print("\n✨ Updating manifests...");

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

    local_manifest.world.inner.address = Some(migration_output.world_address);
    if let Some(salt) = salt {
        local_manifest.world.inner.seed = Some(salt.to_owned());
    }

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
                .find(|c| c.name == output.name.as_ref().unwrap())
                .expect("contract got migrated, means it should be present here");

            let salt = generate_salt(&local.name);
            local.inner.address = Some(get_contract_address(
                salt,
                output.base_class_hash,
                &[],
                migration_output.world_address,
            ));

            local.inner.base_class_hash = output.base_class_hash;
        }
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

pub async fn apply_diff<P, S>(
    ws: &Workspace<'_>,
    account: &SingleOwnerAccount<P, S>,
    txn_config: Option<TxConfig>,
    strategy: &mut MigrationStrategy,
) -> Result<MigrationOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();

    println!("  ");

    let migration_output = execute_strategy(ws, strategy, account, txn_config)
        .await
        .map_err(|e| anyhow!(e))
        .with_context(|| "Problem trying to migrate.")?;

    if migration_output.full {
        if let Some(block_number) = migration_output.world_block_number {
            ui.print(format!(
                "\n🎉 Successfully migrated World on block #{} at address {}",
                block_number,
                bold_message(format!(
                    "{:#x}",
                    strategy.world_address().expect("world address must exist")
                ))
            ));
        } else {
            ui.print(format!(
                "\n🎉 Successfully migrated World at address {}",
                bold_message(format!(
                    "{:#x}",
                    strategy.world_address().expect("world address must exist")
                ))
            ));
        }
    } else {
        ui.print(format!(
            "\n🚨 Partially migrated World at address {}",
            bold_message(format!(
                "{:#x}",
                strategy.world_address().expect("world address must exist")
            ))
        ));
    }

    Ok(migration_output)
}

async fn load_world_manifests<P, S>(
    profile_dir: &Utf8PathBuf,
    account: &SingleOwnerAccount<P, S>,
    world_address: Option<FieldElement>,
    ui: &Ui,
) -> Result<(BaseManifest, Option<DeploymentManifest>)>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    ui.print_step(1, "🌎", "Building World state...");

    let mut local_manifest = BaseManifest::load_from_path(&profile_dir.join(BASE_DIR))
        .map_err(|e| anyhow!("Fail to load local manifest file: {e}."))?;

    let overlay_path = profile_dir.join(OVERLAYS_DIR);
    if overlay_path.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(&profile_dir.join(OVERLAYS_DIR))
            .map_err(|e| anyhow!("Fail to load overlay manifest file: {e}."))?;

        // merge user defined changes to base manifest
        local_manifest.merge(overlay_manifest);
    }

    let remote_manifest = if let Some(address) = world_address {
        match DeploymentManifest::load_from_remote(account.provider(), address).await {
            Ok(manifest) => {
                ui.print_sub(format!("Found remote World: {address:#x}"));
                Some(manifest)
            }
            Err(AbstractManifestError::RemoteWorldNotFound) => None,
            Err(e) => {
                ui.verbose(format!("{e:?}"));
                return Err(anyhow!("Failed to build remote World state: {e}"));
            }
        }
    } else {
        None
    };

    if remote_manifest.is_none() {
        ui.print_sub("No remote World found");
    }

    Ok((local_manifest, remote_manifest))
}

pub fn prepare_migration(
    target_dir: &Utf8PathBuf,
    diff: WorldDiff,
    name: Option<String>,
    world_address: Option<FieldElement>,
    ui: &Ui,
) -> Result<MigrationStrategy> {
    ui.print_step(3, "📦", "Preparing for migration...");

    if name.is_none() && !diff.world.is_same() {
        bail!(
            "World name is required when attempting to migrate the World contract. Please provide \
             it using `--name`."
        );
    }

    let name = if let Some(name) = name {
        Some(cairo_short_string_to_felt(&name).with_context(|| "Failed to parse World name.")?)
    } else {
        None
    };

    let migration = prepare_for_migration(world_address, name, target_dir, diff)
        .with_context(|| "Problem preparing for migration.")?;

    let info = migration.info();

    ui.print_sub(format!(
        "Total items to be migrated ({}): New {} Update {}",
        info.new + info.update,
        info.new,
        info.update
    ));

    Ok(migration)
}

pub async fn execute_strategy<P, S>(
    ws: &Workspace<'_>,
    strategy: &mut MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    txn_config: Option<TxConfig>,
) -> Result<MigrationOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();
    let mut world_tx_hash: Option<FieldElement> = None;
    let mut world_block_number: Option<u64> = None;

    match &strategy.base {
        Some(base) => {
            ui.print_header("# Base Contract");

            match base.declare(migrator, txn_config.unwrap_or_default()).await {
                Ok(res) => {
                    ui.print_sub(format!("Class Hash: {:#x}", res.class_hash));
                }
                Err(MigrationError::ClassAlreadyDeclared) => {
                    ui.print_sub(format!("Already declared: {:#x}", base.diff.local));
                }
                Err(MigrationError::ArtifactError(e)) => {
                    return Err(handle_artifact_error(&ui, base.artifact_path(), e));
                }
                Err(e) => {
                    ui.verbose(format!("{e:?}"));
                    return Err(e.into());
                }
            };
        }
        None => {}
    };

    match &strategy.world {
        Some(world) => {
            ui.print_header("# World");

            // If a migration is pending for the world, we upgrade only if the remote world
            // already exists.
            if world.diff.remote_class_hash.is_some() {
                let _deploy_result = upgrade_contract(
                    world,
                    "world",
                    world.diff.original_class_hash,
                    strategy.base.as_ref().unwrap().diff.original,
                    migrator,
                    &ui,
                    &txn_config,
                )
                .await
                .map_err(|e| {
                    ui.verbose(format!("{e:?}"));
                    anyhow!("Failed to upgrade world: {e}")
                })?;

                ui.print_sub(format!(
                    "Upgraded Contract at address: {:#x}",
                    world.contract_address
                ));
            } else {
                let calldata = vec![strategy.base.as_ref().unwrap().diff.local];
                let deploy_result =
                    deploy_contract(world, "world", calldata.clone(), migrator, &ui, &txn_config)
                        .await
                        .map_err(|e| {
                            ui.verbose(format!("{e:?}"));
                            anyhow!("Failed to deploy world: {e}")
                        })?;

                (world_tx_hash, world_block_number) =
                    if let ContractDeploymentOutput::Output(deploy_result) = deploy_result {
                        (Some(deploy_result.transaction_hash), deploy_result.block_number)
                    } else {
                        (None, None)
                    };

                ui.print_sub(format!("Contract address: {:#x}", world.contract_address));

                let offline = ws.config().offline();

                if offline {
                    ui.print_sub("Skipping metadata upload because of offline mode");
                } else {
                    upload_metadata(ws, world, migrator, &ui).await?;
                }
            }
        }
        None => {}
    };

    let mut migration_output = MigrationOutput {
        world_address: strategy.world_address()?,
        world_tx_hash,
        world_block_number,
        full: false,
        contracts: vec![],
    };

    // Once Torii supports indexing arrays, we should declare and register the
    // ResourceMetadata model.

    match register_models(strategy, migrator, &ui, txn_config).await {
        Ok(_) => (),
        Err(e) => {
            ui.anyhow(&e);
            return Ok(migration_output);
        }
    }

    match deploy_dojo_contracts(strategy, migrator, &ui, txn_config).await {
        Ok(res) => {
            migration_output.contracts = res;
        }
        Err(e) => {
            ui.anyhow(&e);
            return Ok(migration_output);
        }
    };

    migration_output.full = true;

    Ok(migration_output)
}

async fn upload_metadata<P, S>(
    ws: &Workspace<'_>,
    world: &ContractMigration,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
) -> Result<(), anyhow::Error>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let metadata = dojo_metadata_from_workspace(ws);
    if let Some(meta) = metadata.as_ref().and_then(|inner| inner.world()) {
        match meta.upload().await {
            Ok(hash) => {
                let mut encoded_uri = cairo_utils::encode_uri(&format!("ipfs://{hash}"))?;

                // Metadata is expecting an array of capacity 3.
                if encoded_uri.len() < 3 {
                    encoded_uri.extend(vec![FieldElement::ZERO; 3 - encoded_uri.len()]);
                }

                let world_metadata =
                    ResourceMetadata { resource_id: FieldElement::ZERO, metadata_uri: encoded_uri };

                let InvokeTransactionResult { transaction_hash } =
                    WorldContract::new(world.contract_address, migrator)
                        .set_metadata(&world_metadata)
                        .send()
                        .await
                        .map_err(|e| {
                            ui.verbose(format!("{e:?}"));
                            anyhow!("Failed to set World metadata: {e}")
                        })?;

                TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

                ui.print_sub(format!("Set Metadata transaction: {:#x}", transaction_hash));
                ui.print_sub(format!("Metadata uri: ipfs://{hash}"));
            }
            Err(err) => {
                ui.print_sub(format!("Failed to set World metadata:\n{err}"));
            }
        }
    }
    Ok(())
}

enum ContractDeploymentOutput {
    AlreadyDeployed(FieldElement),
    Output(DeployOutput),
}

enum ContractUpgradeOutput {
    Output(UpgradeOutput),
}

async fn deploy_contract<P, S>(
    contract: &ContractMigration,
    contract_id: &str,
    constructor_calldata: Vec<FieldElement>,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: &Option<TxConfig>,
) -> Result<ContractDeploymentOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    match contract
        .deploy(
            contract.diff.local_class_hash,
            constructor_calldata,
            migrator,
            txn_config.unwrap_or_default(),
        )
        .await
    {
        Ok(mut val) => {
            if let Some(declare) = val.clone().declare {
                ui.print_hidden_sub(format!(
                    "Declare transaction: {:#x}",
                    declare.transaction_hash
                ));
            }

            ui.print_hidden_sub(format!("Deploy transaction: {:#x}", val.transaction_hash));

            val.name = Some(contract.diff.name.clone());
            Ok(ContractDeploymentOutput::Output(val))
        }
        Err(MigrationError::ContractAlreadyDeployed(contract_address)) => {
            Ok(ContractDeploymentOutput::AlreadyDeployed(contract_address))
        }
        Err(MigrationError::ArtifactError(e)) => {
            return Err(handle_artifact_error(ui, contract.artifact_path(), e));
        }
        Err(e) => {
            ui.verbose(format!("{e:?}"));
            Err(anyhow!("Failed to migrate {contract_id}: {e}"))
        }
    }
}

async fn upgrade_contract<P, S>(
    contract: &ContractMigration,
    contract_id: &str,
    original_class_hash: FieldElement,
    original_base_class_hash: FieldElement,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: &Option<TxConfig>,
) -> Result<ContractUpgradeOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    match contract
        .upgrade_world(
            contract.diff.local_class_hash,
            original_class_hash,
            original_base_class_hash,
            migrator,
            (*txn_config).unwrap_or_default(),
        )
        .await
    {
        Ok(val) => {
            if let Some(declare) = val.clone().declare {
                ui.print_hidden_sub(format!(
                    "Declare transaction: {:#x}",
                    declare.transaction_hash
                ));
            }

            ui.print_hidden_sub(format!("Upgrade transaction: {:#x}", val.transaction_hash));

            Ok(ContractUpgradeOutput::Output(val))
        }
        Err(MigrationError::ArtifactError(e)) => {
            return Err(handle_artifact_error(ui, contract.artifact_path(), e));
        }
        Err(e) => {
            ui.verbose(format!("{e:?}"));
            Err(anyhow!("Failed to upgrade {contract_id}: {e}"))
        }
    }
}

async fn register_models<P, S>(
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: Option<TxConfig>,
) -> Result<Option<RegisterOutput>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let models = &strategy.models;

    if models.is_empty() {
        return Ok(None);
    }

    ui.print_header(format!("# Models ({})", models.len()));

    let mut declare_output = vec![];

    for c in models.iter() {
        ui.print(italic_message(&c.diff.name).to_string());

        let res = c.declare(migrator, txn_config.unwrap_or_default()).await;
        match res {
            Ok(output) => {
                ui.print_hidden_sub(format!("Declare transaction: {:#x}", output.transaction_hash));

                declare_output.push(output);
            }

            // Continue if model is already declared
            Err(MigrationError::ClassAlreadyDeclared) => {
                ui.print_sub(format!("Already declared: {:#x}", c.diff.local));
                continue;
            }
            Err(MigrationError::ArtifactError(e)) => {
                return Err(handle_artifact_error(ui, c.artifact_path(), e));
            }
            Err(e) => {
                ui.verbose(format!("{e:?}"));
                bail!("Failed to declare model {}: {e}", c.diff.name)
            }
        }

        ui.print_sub(format!("Class hash: {:#x}", c.diff.local));
    }

    let world_address = strategy.world_address()?;
    let world = WorldContract::new(world_address, migrator);

    let calls = models
        .iter()
        .map(|c| world.register_model_getcall(&c.diff.local.into()))
        .collect::<Vec<_>>();

    let InvokeTransactionResult { transaction_hash } =
        migrator.execute(calls).send().await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register models to World: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All models are registered at: {transaction_hash:#x}"));

    Ok(Some(RegisterOutput { transaction_hash, declare_output }))
}

async fn deploy_dojo_contracts<P, S>(
    strategy: &mut MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: Option<TxConfig>,
) -> Result<Vec<Option<DeployOutput>>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let contracts = &strategy.contracts;

    if contracts.is_empty() {
        return Ok(vec![]);
    }

    ui.print_header(format!("# Contracts ({})", contracts.len()));

    let mut deploy_output = vec![];

    let world_address = strategy.world_address()?;

    let contracts = &mut strategy.contracts;
    for contract in contracts {
        let name = &contract.diff.name;
        ui.print(italic_message(name).to_string());
        match contract
            .deploy_dojo_contract(
                world_address,
                contract.diff.local_class_hash,
                contract.diff.base_class_hash,
                migrator,
                txn_config.unwrap_or_default(),
            )
            .await
        {
            Ok(mut output) => {
                if let Some(ref declare) = output.declare {
                    ui.print_hidden_sub(format!(
                        "Declare transaction: {:#x}",
                        declare.transaction_hash
                    ));
                }

                contract.contract_address = output.contract_address;

                if output.was_upgraded {
                    ui.print_hidden_sub(format!(
                        "Invoke transaction to upgrade: {:#x}",
                        output.transaction_hash
                    ));
                    ui.print_sub(format!(
                        "Contract address [upgraded]: {:#x}",
                        output.contract_address
                    ));
                } else {
                    ui.print_hidden_sub(format!(
                        "Deploy transaction: {:#x}",
                        output.transaction_hash
                    ));
                    ui.print_sub(format!("Contract address: {:#x}", output.contract_address));
                }
                let name = contract.diff.name.clone();

                output.name = Some(name);
                deploy_output.push(Some(output));
            }
            Err(MigrationError::ContractAlreadyDeployed(contract_address)) => {
                ui.print_sub(format!("Already deployed: {:#x}", contract_address));
                deploy_output.push(None);
            }
            Err(MigrationError::ArtifactError(e)) => {
                return Err(handle_artifact_error(ui, contract.artifact_path(), e));
            }
            Err(e) => {
                ui.verbose(format!("{e:?}"));
                return Err(anyhow!("Failed to migrate {name}: {e}"));
            }
        }
    }

    Ok(deploy_output)
}

pub fn handle_artifact_error(ui: &Ui, artifact_path: &Path, error: anyhow::Error) -> anyhow::Error {
    let path = artifact_path.to_string_lossy();
    let name = artifact_path.file_name().unwrap().to_string_lossy();
    ui.verbose(format!("{path}: {error:?}"));

    anyhow!(
        "Discrepancy detected in {name}.\nUse `sozo clean` to clean your project or `sozo clean \
         --artifacts` to clean artifacts only.\nThen, rebuild your project with `sozo build`."
    )
}

pub async fn get_contract_operation_name<P>(
    provider: &P,
    contract: &ContractMigration,
    world_address: Option<FieldElement>,
) -> String
where
    P: Provider + Sync + Send + 'static,
{
    if let Some(world_address) = world_address {
        if let Ok(base_class_hash) = provider
            .call(
                FunctionCall {
                    contract_address: world_address,
                    calldata: vec![],
                    entry_point_selector: get_selector_from_name("base").unwrap(),
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
        {
            let contract_address =
                get_contract_address(contract.salt, base_class_hash[0], &[], world_address);

            match provider
                .get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address)
                .await
            {
                Ok(current_class_hash) if current_class_hash != contract.diff.local_class_hash => {
                    return format!("upgrade {}", contract.diff.name);
                }
                Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                    return format!("deploy {}", contract.diff.name);
                }
                Ok(_) => return "already deployed".to_string(),
                Err(_) => return format!("deploy {}", contract.diff.name),
            }
        }
    }
    format!("deploy {}", contract.diff.name)
}

pub async fn print_strategy<P>(ui: &Ui, provider: &P, strategy: &MigrationStrategy)
where
    P: Provider + Sync + Send + 'static,
{
    ui.print("\n📋 Migration Strategy\n");

    if let Some(base) = &strategy.base {
        ui.print_header("# Base Contract");
        ui.print_sub(format!("declare (class hash: {:#x})\n", base.diff.local));
    }

    if let Some(world) = &strategy.world {
        ui.print_header("# World");
        ui.print_sub(format!("declare (class hash: {:#x})\n", world.diff.local_class_hash));
    }

    if !&strategy.models.is_empty() {
        ui.print_header(format!("# Models ({})", &strategy.models.len()));
        for m in &strategy.models {
            ui.print_sub(format!("register {} (class hash: {:#x})", m.diff.name, m.diff.local));
        }
        ui.print(" ");
    }

    if !&strategy.contracts.is_empty() {
        ui.print_header(format!("# Contracts ({})", &strategy.contracts.len()));
        for c in &strategy.contracts {
            let op_name = get_contract_operation_name(provider, c, strategy.world_address).await;
            ui.print_sub(format!("{op_name} (class hash: {:#x})", c.diff.local_class_hash));
        }
        ui.print(" ");
    }
}
