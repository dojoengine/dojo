use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8PathBuf;
use dojo_world::contracts::abi::world;
use dojo_world::contracts::{cairo_utils, WorldContract};
use dojo_world::manifest::{
    AbiFormat, BaseManifest, DeploymentManifest, DojoContract, DojoModel, Manifest,
    ManifestMethods, WorldContract as ManifestWorldContract, WorldMetadata, ABIS_DIR, BASE_DIR,
    DEPLOYMENTS_DIR, MANIFESTS_DIR,
};
use dojo_world::metadata::{dojo_metadata_from_workspace, ResourceMetadata};
use dojo_world::migration::class::ClassMigration;
use dojo_world::migration::contract::ContractMigration;
use dojo_world::migration::strategy::{generate_salt, prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{
    Declarable, Deployable, MigrationError, RegisterOutput, TxnConfig, Upgradable,
};
use dojo_world::utils::{TransactionExt, TransactionWaiter};
use futures::future;
use scarb::core::Workspace;
use scarb_ui::Ui;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, FunctionCall, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_contract_address, get_selector_from_name,
};
use starknet::providers::{Provider, ProviderError};
use starknet::signers::Signer;
use starknet_crypto::FieldElement;
use tokio::fs;

use super::ui::{bold_message, italic_message, MigrationUi};
use super::{
    ContractDeploymentOutput, ContractMigrationOutput, ContractUpgradeOutput, MigrationOutput,
};

pub fn prepare_migration(
    target_dir: &Utf8PathBuf,
    diff: WorldDiff,
    name: &str,
    world_address: Option<FieldElement>,
    ui: &Ui,
) -> Result<MigrationStrategy> {
    ui.print_step(3, "📦", "Preparing for migration...");

    let name = cairo_short_string_to_felt(name).with_context(|| "Failed to parse World name.")?;

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

pub async fn apply_diff<P, S>(
    ws: &Workspace<'_>,
    account: &SingleOwnerAccount<P, S>,
    txn_config: TxnConfig,
    strategy: &mut MigrationStrategy,
) -> Result<MigrationOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();

    ui.print_step(4, "🛠", "Migrating...");
    ui.print(" ");

    let migration_output = execute_strategy(ws, strategy, account, txn_config)
        .await
        .map_err(|e| anyhow!(e))
        .with_context(|| "Problem trying to migrate.")?;

    if migration_output.full {
        if let Some(block_number) = migration_output.world_block_number {
            ui.print(format!(
                "\n🎉 Successfully migrated World on block #{} at address {}\n",
                block_number,
                bold_message(format!(
                    "{:#x}",
                    strategy.world_address().expect("world address must exist")
                ))
            ));
        } else {
            ui.print(format!(
                "\n🎉 Successfully migrated World at address {}\n",
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

pub async fn execute_strategy<P, S>(
    ws: &Workspace<'_>,
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    txn_config: TxnConfig,
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

            match base.declare(migrator, &txn_config).await {
                Ok(res) => {
                    ui.print_sub(format!("Class Hash: {:#x}", res.class_hash));
                }
                Err(MigrationError::ClassAlreadyDeclared) => {
                    ui.print_sub(format!("Already declared: {:#x}", base.diff.local_class_hash));
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
                    strategy.base.as_ref().unwrap().diff.original_class_hash,
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
                let calldata = vec![strategy.base.as_ref().unwrap().diff.local_class_hash];
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
            }
        }
        None => {}
    };

    let mut migration_output = MigrationOutput {
        world_address: strategy.world_address()?,
        world_tx_hash,
        world_block_number,
        full: false,
        models: vec![],
        contracts: vec![],
    };

    let world_address = strategy.world_address()?;

    // Once Torii supports indexing arrays, we should declare and register the
    // ResourceMetadata model.
    match register_dojo_models(&strategy.models, world_address, migrator, &ui, &txn_config).await {
        Ok(output) => {
            migration_output.models = output.registered_model_names;
        }
        Err(e) => {
            ui.anyhow(&e);
            return Ok(migration_output);
        }
    };

    match register_dojo_contracts(&strategy.contracts, world_address, migrator, &ui, &txn_config)
        .await
    {
        Ok(output) => {
            migration_output.contracts = output;
        }
        Err(e) => {
            ui.anyhow(&e);
            return Ok(migration_output);
        }
    };

    migration_output.full = true;

    Ok(migration_output)
}

/// Upload a metadata as a IPFS artifact and then create a resource to register
/// into the Dojo resource registry.
///
/// # Arguments
/// * `element_name` - fully qualified name of the element linked to the metadata
/// * `resource_id` - the id of the resource to create.
/// * `artifact` - the artifact to upload on IPFS.
///
/// # Returns
/// A [`ResourceData`] object to register in the Dojo resource register
/// on success.
async fn upload_on_ipfs_and_create_resource(
    ui: &Ui,
    resource_id: FieldElement,
    metadata: ResourceMetadata,
) -> Result<world::ResourceMetadata> {
    match metadata.upload().await {
        Ok(hash) => {
            ui.print_sub(format!("{}: ipfs://{}", metadata.name, hash));
            create_resource_metadata(resource_id, hash)
        }
        Err(_) => Err(anyhow!("Failed to upload IPFS resource.")),
    }
}

/// Create a resource to register in the Dojo resource registry.
///
/// # Arguments
/// * `resource_id` - the ID of the resource
/// * `hash` - the IPFS hash
///
/// # Returns
/// A [`ResourceData`] object to register in the Dojo resource register
/// on success.
fn create_resource_metadata(
    resource_id: FieldElement,
    hash: String,
) -> Result<world::ResourceMetadata> {
    let metadata_uri = cairo_utils::encode_uri(&format!("ipfs://{hash}"))?;
    Ok(world::ResourceMetadata { resource_id, metadata_uri })
}

/// Upload metadata of the world/models/contracts as IPFS artifacts and then
/// register them in the Dojo resource registry.
///
/// # Arguments
///
/// * `ws` - the workspace
/// * `migrator` - the account used to migrate
/// * `migration_output` - the output after having applied the migration plan.
pub async fn upload_metadata<P, S>(
    ws: &Workspace<'_>,
    migrator: &SingleOwnerAccount<P, S>,
    migration_output: MigrationOutput,
    txn_config: TxnConfig,
) -> Result<()>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();

    ui.print(" ");
    ui.print_step(7, "🌐", "Uploading metadata...");
    ui.print(" ");

    let dojo_metadata = if let Some(metadata) = dojo_metadata_from_workspace(ws) {
        metadata
    } else {
        return Err(anyhow!("No current package with dojo metadata found."));
    };
    let mut ipfs = vec![];
    let mut resources = vec![];

    // world
    if migration_output.world_tx_hash.is_some() {
        match dojo_metadata.world.upload().await {
            Ok(hash) => {
                let resource = create_resource_metadata(FieldElement::ZERO, hash.clone())?;
                ui.print_sub(format!("world: ipfs://{}", hash));
                resources.push(resource);
            }
            Err(err) => {
                ui.print_sub(format!("Failed to upload World metadata:\n{err}"));
            }
        }
    }

    // models
    if !migration_output.models.is_empty() {
        for model_name in migration_output.models {
            if let Some(m) = dojo_metadata.resources_artifacts.get(&model_name) {
                ipfs.push(upload_on_ipfs_and_create_resource(
                    &ui,
                    get_selector_from_name(&model_name).expect("ASCII model name"),
                    m.clone(),
                ));
            }
        }
    }

    // contracts
    let migrated_contracts = migration_output.contracts.into_iter().flatten().collect::<Vec<_>>();

    if !migrated_contracts.is_empty() {
        for contract in migrated_contracts {
            if let Some(m) = dojo_metadata.resources_artifacts.get(&contract.name) {
                ipfs.push(upload_on_ipfs_and_create_resource(
                    &ui,
                    contract.contract_address,
                    m.clone(),
                ));
            }
        }
    }

    // upload IPFS
    resources.extend(
        future::try_join_all(ipfs)
            .await
            .map_err(|_| anyhow!("Unable to upload IPFS artifacts."))?,
    );

    ui.print("> All IPFS artifacts have been successfully uploaded.".to_string());

    // update the resource registry
    let world = WorldContract::new(migration_output.world_address, migrator);

    let calls = resources.iter().map(|r| world.set_metadata_getcall(r)).collect::<Vec<_>>();

    let InvokeTransactionResult { transaction_hash } =
        migrator.execute(calls).send_with_cfg(&txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register metadata into the resource registry: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!(
        "> All metadata have been registered in the resource registry (tx hash: \
         {transaction_hash:#x})"
    ));

    ui.print("");
    ui.print("\n✨ Done.");

    Ok(())
}

async fn register_dojo_models<P, S>(
    models: &[ClassMigration],
    world_address: FieldElement,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<RegisterOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    if models.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: FieldElement::ZERO,
            declare_output: vec![],
            registered_model_names: vec![],
        });
    }

    ui.print_header(format!("# Models ({})", models.len()));

    let mut declare_output = vec![];
    let mut registered_model_names = vec![];

    for c in models.iter() {
        ui.print(italic_message(&c.diff.name).to_string());

        let res = c.declare(migrator, txn_config).await;
        match res {
            Ok(output) => {
                ui.print_hidden_sub(format!("Declare transaction: {:#x}", output.transaction_hash));

                declare_output.push(output);
            }

            // Continue if model is already declared
            Err(MigrationError::ClassAlreadyDeclared) => {
                ui.print_sub(format!("Already declared: {:#x}", c.diff.local_class_hash));
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

        ui.print_sub(format!("Class hash: {:#x}", c.diff.local_class_hash));
    }

    let world = WorldContract::new(world_address, migrator);

    let calls = models
        .iter()
        .map(|c| {
            registered_model_names.push(c.diff.name.clone());
            world.register_model_getcall(&c.diff.local_class_hash.into())
        })
        .collect::<Vec<_>>();

    let InvokeTransactionResult { transaction_hash } =
        world.account.execute(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register models to World: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All models are registered at: {transaction_hash:#x}"));

    Ok(RegisterOutput { transaction_hash, declare_output, registered_model_names })
}

async fn register_dojo_contracts<P, S>(
    contracts: &Vec<ContractMigration>,
    world_address: FieldElement,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<Vec<Option<ContractMigrationOutput>>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    if contracts.is_empty() {
        return Ok(vec![]);
    }

    ui.print_header(format!("# Contracts ({})", contracts.len()));

    let mut deploy_output = vec![];

    for contract in contracts {
        let name = &contract.diff.name;
        ui.print(italic_message(name).to_string());
        match contract
            .deploy_dojo_contract(
                world_address,
                contract.diff.local_class_hash,
                contract.diff.base_class_hash,
                migrator,
                txn_config,
                &contract.diff.init_calldata,
            )
            .await
        {
            Ok(output) => {
                if let Some(ref declare) = output.declare {
                    ui.print_hidden_sub(format!(
                        "Declare transaction: {:#x}",
                        declare.transaction_hash
                    ));
                }

                // NOTE: this assignment may not look useful since we are dropping
                // `MigrationStrategy` without actually using this value from it.
                // but some tests depend on this behaviour
                // contract.contract_address = output.contract_address;

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
                deploy_output.push(Some(ContractMigrationOutput {
                    name: name.to_string(),
                    contract_address: output.contract_address,
                    base_class_hash: output.base_class_hash,
                }));
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
                return Err(anyhow!(
                    "Failed to migrate {name}: {e}. Please also verify init calldata is valid, if \
                     any."
                ));
            }
        }
    }

    Ok(deploy_output)
}

async fn deploy_contract<P, S>(
    contract: &ContractMigration,
    contract_id: &str,
    constructor_calldata: Vec<FieldElement>,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<ContractDeploymentOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    match contract
        .deploy(contract.diff.local_class_hash, constructor_calldata, migrator, txn_config)
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
    txn_config: &TxnConfig,
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
            txn_config,
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

pub fn handle_artifact_error(ui: &Ui, artifact_path: &Path, error: anyhow::Error) -> anyhow::Error {
    let path = artifact_path.to_string_lossy();
    let name = artifact_path.file_name().unwrap().to_string_lossy();
    ui.verbose(format!("{path}: {error:?}"));

    anyhow!(
        "Discrepancy detected in {name}.\nUse `sozo clean` to clean your project.\n
        Then, rebuild your project with `sozo build`."
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
                    return format!("{}: Upgrade", contract.diff.name);
                }
                Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                    return format!("{}: Deploy", contract.diff.name);
                }
                Ok(_) => return "Already Deployed".to_string(),
                Err(_) => return format!("{}: Deploy", contract.diff.name),
            }
        }
    }
    format!("deploy {}", contract.diff.name)
}

pub async fn print_strategy<P>(
    ui: &Ui,
    provider: &P,
    strategy: &MigrationStrategy,
    world_address: FieldElement,
) where
    P: Provider + Sync + Send + 'static,
{
    ui.print("\n📋 Migration Strategy\n");

    ui.print_header(format!("World address: {:#x}", world_address));

    ui.print(" ");

    if let Some(base) = &strategy.base {
        ui.print_header("# Base Contract");
        ui.print_sub(format!("Class hash: {:#x}", base.diff.local_class_hash));
    }

    ui.print(" ");

    if let Some(world) = &strategy.world {
        ui.print_header("# World");
        ui.print_sub(format!("Class hash: {:#x}", world.diff.local_class_hash));
    }

    ui.print(" ");

    if !&strategy.models.is_empty() {
        ui.print_header(format!("# Models ({})", &strategy.models.len()));
        for m in &strategy.models {
            ui.print(m.diff.name.to_string());
            ui.print_sub(format!("Class hash: {:#x}", m.diff.local_class_hash));
        }
    }

    ui.print(" ");

    if !&strategy.contracts.is_empty() {
        ui.print_header(format!("# Contracts ({})", &strategy.contracts.len()));
        for c in &strategy.contracts {
            let op_name = get_contract_operation_name(provider, c, strategy.world_address).await;

            ui.print(op_name);
            ui.print_sub(format!("Class hash: {:#x}", c.diff.local_class_hash));
            let salt = generate_salt(&c.diff.name);
            let contract_address =
                get_contract_address(salt, c.diff.base_class_hash, &[], world_address);
            ui.print_sub(format!("Contract address: {:#x}", contract_address));
        }
    }

    ui.print(" ");
}

#[allow(clippy::too_many_arguments)]
pub async fn update_manifests_and_abis(
    ws: &Workspace<'_>,
    local_manifest: BaseManifest,
    manifest_dir: &Utf8PathBuf,
    profile_name: &str,
    rpc_url: &str,
    world_address: FieldElement,
    migration_output: Option<MigrationOutput>,
    salt: &str,
) -> Result<()> {
    let ui = ws.config().ui();
    ui.print_step(5, "✨", "Updating manifests...");

    let deployed_path =
        manifest_dir.join(MANIFESTS_DIR).join(profile_name).join("manifest").with_extension("toml");
    let deployed_path_json =
        manifest_dir.join(MANIFESTS_DIR).join(profile_name).join("manifest").with_extension("json");

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
    local_manifest.world.inner.seed = salt.to_owned();

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
        if contract.inner.base_class_hash != FieldElement::ZERO {
            let salt = generate_salt(&contract.name);
            contract.inner.address = Some(get_contract_address(
                salt,
                contract.inner.base_class_hash,
                &[],
                world_address,
            ));
        }
    });

    // copy abi files from `abi/base` to `abi/deployments/{chain_id}` and update abi path in
    // local_manifest
    update_manifest_abis(&mut local_manifest, manifest_dir, profile_name).await;

    local_manifest.write_to_path_toml(&deployed_path)?;
    local_manifest.write_to_path_json(&deployed_path_json, manifest_dir)?;
    ui.print("\n✨ Done.");

    Ok(())
}

async fn update_manifest_abis(
    local_manifest: &mut DeploymentManifest,
    manifest_dir: &Utf8PathBuf,
    profile_name: &str,
) {
    fs::create_dir_all(
        manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(ABIS_DIR).join(DEPLOYMENTS_DIR),
    )
    .await
    .expect("Failed to create folder");

    async fn inner_helper<T>(
        manifest_dir: &Utf8PathBuf,
        profile_name: &str,
        manifest: &mut Manifest<T>,
    ) where
        T: ManifestMethods,
    {
        // for example:
        // from: manifests/dev/abis/base/contract/dojo_world_world.json
        // to: manifests/dev/abis/deployments/contract/dojo_world_world.json
        //
        // Unwraps in call to abi is safe because we always write abis for DojoContracts as relative
        // path.
        // In this relative path, we only what the root from
        // ABI directory.

        // manifests/dev/abis/base/contract/dojo_world_world.json
        let base_relative_path = manifest.inner.abi().unwrap().to_path().unwrap();

        // contract/dojo_world_world.json
        let stripped_path = base_relative_path
            .strip_prefix(
                Utf8PathBuf::new()
                    .join(MANIFESTS_DIR)
                    .join(profile_name)
                    .join(ABIS_DIR)
                    .join(BASE_DIR),
            )
            .unwrap();

        let deployed_relative_path = Utf8PathBuf::new()
            .join(MANIFESTS_DIR)
            .join(profile_name)
            .join(ABIS_DIR)
            .join(DEPLOYMENTS_DIR)
            .join(stripped_path);

        let full_base_path = manifest_dir.join(base_relative_path);
        let full_deployed_path = manifest_dir.join(deployed_relative_path.clone());

        fs::create_dir_all(full_deployed_path.parent().unwrap())
            .await
            .expect("Failed to create folder");

        fs::copy(full_base_path, full_deployed_path).await.expect("Failed to copy abi file");

        manifest.inner.set_abi(Some(AbiFormat::Path(deployed_relative_path)));
    }

    inner_helper::<ManifestWorldContract>(manifest_dir, profile_name, &mut local_manifest.world)
        .await;

    for contract in local_manifest.contracts.iter_mut() {
        inner_helper::<DojoContract>(manifest_dir, profile_name, contract).await;
    }

    for model in local_manifest.models.iter_mut() {
        inner_helper::<DojoModel>(manifest_dir, profile_name, model).await;
    }
}
