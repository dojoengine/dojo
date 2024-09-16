use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use cainome::cairo_serde::ByteArray;
use camino::Utf8PathBuf;
use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use dojo_world::contracts::abi::world::{self, Resource};
use dojo_world::contracts::naming::{
    self, compute_selector_from_tag, get_name_from_tag, get_namespace_from_tag,
};
use dojo_world::contracts::{cairo_utils, WorldContract};
use dojo_world::manifest::{
    AbiFormat, BaseManifest, Class, DeploymentManifest, DojoContract, DojoEvent, DojoModel,
    Manifest, ManifestMethods, WorldContract as ManifestWorldContract, WorldMetadata, ABIS_DIR,
    BASE_DIR, DEPLOYMENT_DIR, MANIFESTS_DIR,
};
use dojo_world::metadata::{dojo_metadata_from_workspace, ResourceMetadata};
use dojo_world::migration::class::ClassMigration;
use dojo_world::migration::contract::ContractMigration;
use dojo_world::migration::strategy::{generate_salt, prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{Declarable, Deployable, MigrationError, RegisterOutput, Upgradable};
use futures::future;
use itertools::Itertools;
use scarb::core::Workspace;
use scarb_ui::Ui;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, Felt, FunctionCall, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_contract_address, get_selector_from_name,
};
use starknet::providers::{AnyProvider, Provider, ProviderError};
use starknet::signers::LocalWallet;
use tokio::fs;

use super::ui::{bold_message, italic_message, MigrationUi};
use super::utils::generate_resource_map;
use super::{
    ContractDeploymentOutput, ContractMigrationOutput, ContractUpgradeOutput, MigrationOutput,
};
use crate::auth::{get_resource_selector, ResourceType, ResourceWriter};

pub fn prepare_migration(
    target_dir: &Utf8PathBuf,
    diff: WorldDiff,
    name: &str,
    world_address: Option<Felt>,
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

pub async fn apply_diff<A>(
    ws: &Workspace<'_>,
    account: A,
    txn_config: TxnConfig,
    strategy: &MigrationStrategy,
    declarers: &[SingleOwnerAccount<AnyProvider, LocalWallet>],
) -> Result<MigrationOutput>
where
    A: ConnectedAccount + Sync + Send,
    <A as ConnectedAccount>::Provider: Send,
    A::SignError: 'static,
{
    let ui = ws.config().ui();

    ui.print_step(4, "🛠", "Migrating...");
    ui.print(" ");

    let migration_output = execute_strategy(ws, strategy, account, txn_config, declarers)
        .await
        .map_err(|e| anyhow!(e))
        .with_context(|| "Problem trying to migrate.")?;

    if migration_output.full {
        if let Some(block_number) = migration_output.world_block_number {
            ui.print(format!(
                "\n🎉 Successfully migrated World on block #{} at address {}\n",
                block_number,
                bold_message(format!("{:#x}", strategy.world_address))
            ));
        } else {
            ui.print(format!(
                "\n🎉 Successfully migrated World at address {}\n",
                bold_message(format!("{:#x}", strategy.world_address))
            ));
        }
    } else {
        ui.print(format!(
            "\n🚨 Partially migrated World at address {}",
            bold_message(format!("{:#x}", strategy.world_address))
        ));
    }

    Ok(migration_output)
}

pub async fn execute_strategy<A>(
    ws: &Workspace<'_>,
    strategy: &MigrationStrategy,
    migrator: A,
    txn_config: TxnConfig,
    declarers: &[SingleOwnerAccount<AnyProvider, LocalWallet>],
) -> Result<MigrationOutput>
where
    A: ConnectedAccount + Sync + Send,
    A::Provider: Send,
    A::SignError: 'static,
{
    let ui = ws.config().ui();
    let mut world_tx_hash: Option<Felt> = None;
    let mut world_block_number: Option<u64> = None;

    if let Some(base) = &strategy.base {
        ui.print_header("# Base Contract");

        match base.declare(&migrator, &txn_config).await {
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

    if let Some(world) = &strategy.world {
        ui.print_header("# World");

        // If a migration is pending for the world, we upgrade only if the remote world
        // already exists.
        if world.diff.remote_class_hash.is_some() {
            let _deploy_result = upgrade_contract(
                world,
                "world",
                world.diff.original_class_hash,
                strategy.base.as_ref().unwrap().diff.original_class_hash,
                &migrator,
                &ui,
                &txn_config,
            )
            .await
            .map_err(|e| {
                ui.verbose(format!("{e:?}"));
                anyhow!("Failed to upgrade world: {e}")
            })?;

            ui.print_sub(format!("Upgraded Contract at address: {:#x}", world.contract_address));
        } else {
            let calldata = vec![strategy.base.as_ref().unwrap().diff.local_class_hash];
            let deploy_result =
                deploy_contract(world, "world", calldata.clone(), &migrator, &ui, &txn_config)
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

    let world_address = strategy.world_address;
    let mut migration_output = MigrationOutput {
        world_address,
        world_tx_hash,
        world_block_number,
        full: false,
        models: vec![],
        events: vec![],
        contracts: vec![],
    };

    // register namespaces
    let mut namespaces =
        strategy.models.iter().map(|m| get_namespace_from_tag(&m.diff.tag)).collect::<Vec<_>>();
    namespaces.extend(
        strategy.contracts.iter().map(|c| get_namespace_from_tag(&c.diff.tag)).collect::<Vec<_>>(),
    );
    namespaces = namespaces.into_iter().unique().collect::<Vec<_>>();

    register_namespaces(&namespaces, world_address, &migrator, &ui, &txn_config).await?;

    // TODO: rework this part when more time.
    if declarers.is_empty() {
        match register_dojo_events(&strategy.events, world_address, &migrator, &ui, &txn_config)
            .await
        {
            Ok(output) => {
                migration_output.events = output.registered_elements;
            }
            Err(e) => {
                ui.anyhow(&e);
                return Ok(migration_output);
            }
        };

        match register_dojo_models(&strategy.models, world_address, &migrator, &ui, &txn_config)
            .await
        {
            Ok(output) => {
                migration_output.models = output.registered_elements;
            }
            Err(e) => {
                ui.anyhow(&e);
                return Ok(migration_output);
            }
        };

        match register_dojo_contracts(
            &strategy.contracts,
            world_address,
            migrator,
            &ui,
            &txn_config,
        )
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
    } else {
        match register_dojo_events_with_declarers(
            &strategy.events,
            world_address,
            &migrator,
            &ui,
            &txn_config,
            declarers,
        )
        .await
        {
            Ok(output) => {
                migration_output.events = output.registered_elements;
            }
            Err(e) => {
                ui.anyhow(&e);
                return Ok(migration_output);
            }
        };

        match register_dojo_models_with_declarers(
            &strategy.models,
            world_address,
            &migrator,
            &ui,
            &txn_config,
            declarers,
        )
        .await
        {
            Ok(output) => {
                migration_output.models = output.registered_elements;
            }
            Err(e) => {
                ui.anyhow(&e);
                return Ok(migration_output);
            }
        };

        match register_dojo_contracts_declarers(
            &strategy.contracts,
            world_address,
            migrator,
            &ui,
            &txn_config,
            declarers,
        )
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
    }

    migration_output.full = true;

    Ok(migration_output)
}

/// Upload a metadata as a IPFS artifact and then create a resource to register
/// into the Dojo resource registry.
///
/// # Arguments
/// * `ui` - The user interface object for displaying information
/// * `resource_id` - The id of the resource to create
/// * `metadata` - The ResourceMetadata object containing the metadata to upload
///
/// # Returns
/// A [`world::ResourceMetadata`] object to register in the Dojo resource register
/// on success, or an error if the upload fails.
async fn upload_on_ipfs_and_create_resource(
    ui: &Ui,
    resource_id: Felt,
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
fn create_resource_metadata(resource_id: Felt, hash: String) -> Result<world::ResourceMetadata> {
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
pub async fn upload_metadata<A>(
    ws: &Workspace<'_>,
    migrator: A,
    migration_output: MigrationOutput,
    txn_config: TxnConfig,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send,
    <A as ConnectedAccount>::Provider: Send,
{
    let ui = ws.config().ui();

    ui.print(" ");
    ui.print_step(8, "🌐", "Uploading metadata...");
    ui.print(" ");

    let dojo_metadata = dojo_metadata_from_workspace(ws)?;
    let mut ipfs = vec![];
    let mut resources = vec![];

    // world
    if migration_output.world_tx_hash.is_some() {
        match dojo_metadata.world.upload().await {
            Ok(hash) => {
                let resource = create_resource_metadata(Felt::ZERO, hash.clone())?;
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
        for model_tag in migration_output.models {
            if let Some(m) = dojo_metadata.resources_artifacts.get(&model_tag) {
                ipfs.push(upload_on_ipfs_and_create_resource(
                    &ui,
                    compute_selector_from_tag(&model_tag),
                    m.clone(),
                ));
            }
        }
    }

    // contracts
    let migrated_contracts = migration_output.contracts.into_iter().flatten().collect::<Vec<_>>();

    if !migrated_contracts.is_empty() {
        for contract in migrated_contracts {
            if let Some(m) = dojo_metadata.resources_artifacts.get(&contract.tag) {
                ipfs.push(upload_on_ipfs_and_create_resource(
                    &ui,
                    naming::compute_selector_from_tag(&contract.tag),
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
    let world = WorldContract::new(migration_output.world_address, &migrator);

    let calls = resources.iter().map(|r| world.set_metadata_getcall(r)).collect::<Vec<_>>();

    if calls.is_empty() {
        ui.print_sub("No metadata to register");
        return Ok(());
    }

    let InvokeTransactionResult { transaction_hash } =
        migrator.execute_v1(calls).send_with_cfg(&txn_config).await.map_err(|e| {
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

async fn register_namespaces<A>(
    namespaces: &[String],
    world_address: Felt,
    migrator: &A,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<()>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
{
    let world = WorldContract::new(world_address, migrator);

    // We need to check if the namespace is not already registered.
    let mut registered_namespaces = vec![];

    for namespace in namespaces {
        let namespace_selector = naming::compute_bytearray_hash(namespace);

        if let Resource::Namespace = world.resource(&namespace_selector).call().await? {
            registered_namespaces.push(namespace);
        }
    }

    let calls = namespaces
        .iter()
        .filter(|ns| !registered_namespaces.contains(ns))
        .map(|ns| {
            ui.print(italic_message(&ns).to_string());
            world.register_namespace_getcall(&ByteArray::from_string(ns).unwrap())
        })
        .collect::<Vec<_>>();

    if calls.is_empty() {
        return Ok(());
    }

    ui.print_header(format!("# Namespaces ({})", namespaces.len() - registered_namespaces.len()));

    let InvokeTransactionResult { transaction_hash } =
        world.account.execute_v1(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register namespace to World: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All namespaces are registered at: {transaction_hash:#x}\n"));

    Ok(())
}

async fn register_dojo_events<A>(
    events: &[ClassMigration],
    world_address: Felt,
    migrator: &A,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<RegisterOutput>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
{
    if events.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    ui.print_header(format!("# Events ({})", events.len()));

    let world = WorldContract::new(world_address, &migrator);

    let mut declare_output = vec![];
    let mut events_to_register = vec![];

    for (i, m) in events.iter().enumerate() {
        let tag = &m.diff.tag;

        ui.print(italic_message(tag).to_string());

        if let Resource::Unregistered =
            world.resource(&compute_selector_from_tag(tag)).call().await?
        {
            events_to_register.push(tag.clone());
        } else {
            ui.print_sub("Already registered");
            continue;
        }

        match m.declare(&migrator, txn_config).await {
            Ok(output) => {
                ui.print_sub(format!("Selector: {:#066x}", compute_selector_from_tag(tag)));
                ui.print_hidden_sub(format!("Class hash: {:#066x}", output.class_hash));
                ui.print_hidden_sub(format!(
                    "Declare transaction: {:#066x}",
                    output.transaction_hash
                ));
                declare_output.push(output);
            }
            Err(MigrationError::ClassAlreadyDeclared) => {
                ui.print_sub("Already declared");
            }
            Err(MigrationError::ArtifactError(e)) => {
                return Err(handle_artifact_error(ui, events[i].artifact_path(), e));
            }
            Err(e) => {
                ui.verbose(format!("{e:?}"));
                bail!("Failed to declare event: {e}")
            }
        }
    }

    let calls = events
        .iter()
        .filter(|m| events_to_register.contains(&m.diff.tag))
        .map(|c| world.register_event_getcall(&c.diff.local_class_hash.into()))
        .collect::<Vec<_>>();

    if calls.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    let InvokeTransactionResult { transaction_hash } =
        world.account.execute_v1(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register events to World: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All events are registered at: {transaction_hash:#x}\n"));

    Ok(RegisterOutput { transaction_hash, declare_output, registered_elements: events_to_register })
}

// For now duplicated because the migrator account is different from the declarers account type.
async fn register_dojo_events_with_declarers<A>(
    events: &[ClassMigration],
    world_address: Felt,
    migrator: &A,
    ui: &Ui,
    txn_config: &TxnConfig,
    declarers: &[SingleOwnerAccount<AnyProvider, LocalWallet>],
) -> Result<RegisterOutput>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
{
    if events.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    ui.print_header(format!("# Events ({})", events.len()));

    let mut declare_output = vec![];
    let mut events_to_register = vec![];

    let mut declarers_tasks = HashMap::new();
    for (i, m) in events.iter().enumerate() {
        let declarer_index = i % declarers.len();
        declarers_tasks
            .entry(declarer_index)
            .or_insert(vec![])
            .push((m.diff.tag.clone(), m.declare(&declarers[declarer_index], txn_config)));
    }

    let mut futures = Vec::new();

    for (declarer_index, d_tasks) in declarers_tasks {
        let future = async move {
            let mut results = Vec::new();
            for (tag, task) in d_tasks {
                let result = task.await;
                results.push((declarer_index, tag, result));
            }
            results
        };

        futures.push(future);
    }

    let all_results = futures::future::join_all(futures).await;

    let world = WorldContract::new(world_address, &migrator);

    for results in all_results {
        for (index, tag, result) in results {
            ui.print(italic_message(&tag).to_string());

            if let Resource::Unregistered =
                world.resource(&compute_selector_from_tag(&tag)).call().await?
            {
                events_to_register.push(tag.clone());
            } else {
                ui.print_sub("Already registered");
                continue;
            }

            match result {
                Ok(output) => {
                    ui.print_sub(format!("Selector: {:#066x}", compute_selector_from_tag(&tag)));
                    ui.print_hidden_sub(format!("Class hash: {:#066x}", output.class_hash));
                    ui.print_hidden_sub(format!(
                        "Declare transaction: {:#066x}",
                        output.transaction_hash
                    ));
                    declare_output.push(output);
                }
                Err(MigrationError::ClassAlreadyDeclared) => {
                    ui.print_sub("Already declared");
                }
                Err(MigrationError::ArtifactError(e)) => {
                    return Err(handle_artifact_error(ui, events[index].artifact_path(), e));
                }
                Err(e) => {
                    ui.verbose(format!("{e:?}"));
                    bail!("Failed to declare event: {e}")
                }
            }
        }
    }

    let calls = events
        .iter()
        .filter(|m| events_to_register.contains(&m.diff.tag))
        .map(|c| world.register_event_getcall(&c.diff.local_class_hash.into()))
        .collect::<Vec<_>>();

    if calls.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    let InvokeTransactionResult { transaction_hash } =
        world.account.execute_v1(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register events to World: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All events are registered at: {transaction_hash:#x}\n"));

    Ok(RegisterOutput { transaction_hash, declare_output, registered_elements: events_to_register })
}

async fn register_dojo_models<A>(
    models: &[ClassMigration],
    world_address: Felt,
    migrator: &A,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<RegisterOutput>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
{
    if models.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    ui.print_header(format!("# Models ({})", models.len()));

    let world = WorldContract::new(world_address, &migrator);

    let mut declare_output = vec![];
    let mut models_to_register = vec![];

    for (i, m) in models.iter().enumerate() {
        let tag = &m.diff.tag;

        ui.print(italic_message(tag).to_string());

        if let Resource::Unregistered =
            world.resource(&compute_selector_from_tag(tag)).call().await?
        {
            models_to_register.push(tag.clone());
        } else {
            ui.print_sub("Already registered");
            continue;
        }

        match m.declare(&migrator, txn_config).await {
            Ok(output) => {
                ui.print_sub(format!("Selector: {:#066x}", compute_selector_from_tag(tag)));
                ui.print_hidden_sub(format!("Class hash: {:#066x}", output.class_hash));
                ui.print_hidden_sub(format!(
                    "Declare transaction: {:#066x}",
                    output.transaction_hash
                ));
                declare_output.push(output);
            }
            Err(MigrationError::ClassAlreadyDeclared) => {
                ui.print_sub("Already declared");
            }
            Err(MigrationError::ArtifactError(e)) => {
                return Err(handle_artifact_error(ui, models[i].artifact_path(), e));
            }
            Err(e) => {
                ui.verbose(format!("{e:?}"));
                bail!("Failed to declare model: {e}")
            }
        }
    }

    let calls = models
        .iter()
        .filter(|m| models_to_register.contains(&m.diff.tag))
        .map(|c| world.register_model_getcall(&c.diff.local_class_hash.into()))
        .collect::<Vec<_>>();

    if calls.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    let InvokeTransactionResult { transaction_hash } =
        world.account.execute_v1(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register models to World: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All models are registered at: {transaction_hash:#x}\n"));

    Ok(RegisterOutput { transaction_hash, declare_output, registered_elements: models_to_register })
}

// For now duplicated because the migrator account is different from the declarers account type.
async fn register_dojo_models_with_declarers<A>(
    models: &[ClassMigration],
    world_address: Felt,
    migrator: &A,
    ui: &Ui,
    txn_config: &TxnConfig,
    declarers: &[SingleOwnerAccount<AnyProvider, LocalWallet>],
) -> Result<RegisterOutput>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
{
    if models.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    ui.print_header(format!("# Models ({})", models.len()));

    let mut declare_output = vec![];
    let mut models_to_register = vec![];

    let mut declarers_tasks = HashMap::new();
    for (i, m) in models.iter().enumerate() {
        let declarer_index = i % declarers.len();
        declarers_tasks
            .entry(declarer_index)
            .or_insert(vec![])
            .push((m.diff.tag.clone(), m.declare(&declarers[declarer_index], txn_config)));
    }

    let mut futures = Vec::new();

    for (declarer_index, d_tasks) in declarers_tasks {
        let future = async move {
            let mut results = Vec::new();
            for (tag, task) in d_tasks {
                let result = task.await;
                results.push((declarer_index, tag, result));
            }
            results
        };

        futures.push(future);
    }

    let all_results = futures::future::join_all(futures).await;

    let world = WorldContract::new(world_address, &migrator);

    for results in all_results {
        for (index, tag, result) in results {
            ui.print(italic_message(&tag).to_string());

            if let Resource::Unregistered =
                world.resource(&compute_selector_from_tag(&tag)).call().await?
            {
                models_to_register.push(tag.clone());
            } else {
                ui.print_sub("Already registered");
                continue;
            }

            match result {
                Ok(output) => {
                    ui.print_sub(format!("Selector: {:#066x}", compute_selector_from_tag(&tag)));
                    ui.print_hidden_sub(format!("Class hash: {:#066x}", output.class_hash));
                    ui.print_hidden_sub(format!(
                        "Declare transaction: {:#066x}",
                        output.transaction_hash
                    ));
                    declare_output.push(output);
                }
                Err(MigrationError::ClassAlreadyDeclared) => {
                    ui.print_sub("Already declared");
                }
                Err(MigrationError::ArtifactError(e)) => {
                    return Err(handle_artifact_error(ui, models[index].artifact_path(), e));
                }
                Err(e) => {
                    ui.verbose(format!("{e:?}"));
                    bail!("Failed to declare model: {e}")
                }
            }
        }
    }

    let calls = models
        .iter()
        .filter(|m| models_to_register.contains(&m.diff.tag))
        .map(|c| world.register_model_getcall(&c.diff.local_class_hash.into()))
        .collect::<Vec<_>>();

    if calls.is_empty() {
        return Ok(RegisterOutput {
            transaction_hash: Felt::ZERO,
            declare_output: vec![],
            registered_elements: vec![],
        });
    }

    let InvokeTransactionResult { transaction_hash } =
        world.account.execute_v1(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to register models to World: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All models are registered at: {transaction_hash:#x}\n"));

    Ok(RegisterOutput { transaction_hash, declare_output, registered_elements: models_to_register })
}

async fn register_dojo_contracts<A>(
    contracts: &Vec<ContractMigration>,
    world_address: Felt,
    migrator: A,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<Vec<Option<ContractMigrationOutput>>>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
{
    if contracts.is_empty() {
        return Ok(vec![]);
    }

    ui.print_header(format!("# Contracts ({})", contracts.len()));

    let mut declare_outputs = vec![];

    for (i, c) in contracts.iter().enumerate() {
        let tag = &c.diff.tag;
        ui.print(italic_message(&tag).to_string());

        match c.declare(&migrator, txn_config).await {
            Ok(output) => {
                ui.print_sub(format!("Selector: {:#066x}", compute_selector_from_tag(tag)));
                ui.print_hidden_sub(format!("Class hash: {:#066x}", output.class_hash));
                ui.print_hidden_sub(format!(
                    "Declare transaction: {:#066x}",
                    output.transaction_hash
                ));
                declare_outputs.push(output);
            }
            Err(MigrationError::ClassAlreadyDeclared) => {
                ui.print_sub("Already declared");
            }
            Err(MigrationError::ArtifactError(e)) => {
                return Err(handle_artifact_error(ui, contracts[i].artifact_path(), e));
            }
            Err(e) => {
                ui.verbose(format!("{e:?}"));
                bail!("Failed to declare model: {e}")
            }
        }
    }

    let mut calls = vec![];
    let mut deploy_outputs = vec![];

    for contract in contracts {
        let tag = &contract.diff.tag;
        ui.print(italic_message(tag).to_string());

        if let Ok((call, contract_address, was_upgraded)) = contract
            .deploy_dojo_contract_call(
                world_address,
                contract.diff.local_class_hash,
                contract.diff.base_class_hash,
                &migrator,
                tag,
            )
            .await
        {
            let base_class_hash = contract.diff.base_class_hash;

            calls.push(call);

            if was_upgraded {
                ui.print_hidden_sub(format!("{} upgraded at {:#066x}", tag, contract_address));
            } else {
                ui.print_hidden_sub(format!("{} deployed at {:#066x}", tag, contract_address));
            }

            deploy_outputs.push(Some(ContractMigrationOutput {
                tag: tag.clone(),
                contract_address,
                base_class_hash,
                was_upgraded,
            }));
        } else {
            // contract already deployed.
            deploy_outputs.push(None);
        }
    }

    if calls.is_empty() {
        return Ok(deploy_outputs);
    }

    let InvokeTransactionResult { transaction_hash } =
        migrator.execute_v1(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to deploy contracts: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All contracts are deployed at: {transaction_hash:#x}\n"));

    Ok(deploy_outputs)
}

async fn register_dojo_contracts_declarers<A>(
    contracts: &Vec<ContractMigration>,
    world_address: Felt,
    migrator: A,
    ui: &Ui,
    txn_config: &TxnConfig,
    declarers: &[SingleOwnerAccount<AnyProvider, LocalWallet>],
) -> Result<Vec<Option<ContractMigrationOutput>>>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
{
    if contracts.is_empty() {
        return Ok(vec![]);
    }

    ui.print_header(format!("# Contracts ({})", contracts.len()));

    // Declare all and keep (tg, class_hash, tx_hash).
    // Then multicall the deploy matching the class hash.
    let mut declarers_tasks = HashMap::new();
    for (i, c) in contracts.iter().enumerate() {
        let declarer_index = i % declarers.len();
        declarers_tasks
            .entry(declarer_index)
            .or_insert(vec![])
            .push((c.diff.tag.clone(), c.declare(&declarers[declarer_index], txn_config)));
    }

    let mut futures = Vec::new();

    for (declarer_index, d_tasks) in declarers_tasks {
        let future = async move {
            let mut results = Vec::new();
            for (tag, task) in d_tasks {
                let result = task.await;
                results.push((declarer_index, tag, result));
            }
            results
        };

        futures.push(future);
    }

    let all_results = futures::future::join_all(futures).await;

    let mut declare_outputs = vec![];

    for results in all_results {
        for (index, tag, result) in results {
            ui.print(italic_message(&tag).to_string());
            match result {
                Ok(output) => {
                    ui.print_sub(format!("Selector: {:#066x}", compute_selector_from_tag(&tag)));
                    ui.print_hidden_sub(format!("Class hash: {:#066x}", output.class_hash));
                    ui.print_hidden_sub(format!(
                        "Declare transaction: {:#066x}",
                        output.transaction_hash
                    ));
                    declare_outputs.push(output);
                }
                Err(MigrationError::ClassAlreadyDeclared) => {
                    ui.print_sub("Already declared");
                }
                Err(MigrationError::ArtifactError(e)) => {
                    return Err(handle_artifact_error(ui, contracts[index].artifact_path(), e));
                }
                Err(e) => {
                    ui.verbose(format!("{e:?}"));
                    bail!("Failed to declare model: {e}")
                }
            }
        }
    }

    let mut calls = vec![];
    let mut deploy_outputs = vec![];

    for contract in contracts {
        let tag = &contract.diff.tag;
        ui.print(italic_message(tag).to_string());

        if let Ok((call, contract_address, was_upgraded)) = contract
            .deploy_dojo_contract_call(
                world_address,
                contract.diff.local_class_hash,
                contract.diff.base_class_hash,
                &migrator,
                tag,
            )
            .await
        {
            let base_class_hash = contract.diff.base_class_hash;

            calls.push(call);

            if was_upgraded {
                ui.print_sub(format!("{} upgraded at {:#066x}", tag, contract_address));
            } else {
                ui.print_sub(format!("{} deployed at {:#066x}", tag, contract_address));
            }

            deploy_outputs.push(Some(ContractMigrationOutput {
                tag: tag.clone(),
                contract_address,
                base_class_hash,
                was_upgraded,
            }));
        } else {
            // contract already deployed.
            deploy_outputs.push(None);
        }
    }

    if calls.is_empty() {
        return Ok(deploy_outputs);
    }

    let InvokeTransactionResult { transaction_hash } =
        migrator.execute_v1(calls).send_with_cfg(txn_config).await.map_err(|e| {
            ui.verbose(format!("{e:?}"));
            anyhow!("Failed to deploy contracts: {e}")
        })?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print(format!("All contracts are deployed at: {transaction_hash:#x}\n"));

    Ok(deploy_outputs)
}

async fn deploy_contract<A>(
    contract: &ContractMigration,
    contract_id: &str,
    constructor_calldata: Vec<Felt>,
    migrator: A,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<ContractDeploymentOutput>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
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

            val.tag = Some(contract.diff.tag.clone());
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

async fn upgrade_contract<A>(
    contract: &ContractMigration,
    contract_id: &str,
    original_class_hash: Felt,
    original_base_class_hash: Felt,
    migrator: A,
    ui: &Ui,
    txn_config: &TxnConfig,
) -> Result<ContractUpgradeOutput>
where
    A: ConnectedAccount + Send + Sync,
    <A as ConnectedAccount>::Provider: Send,
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
    provider: P,
    contract: &ContractMigration,
    world_address: Felt,
) -> String
where
    P: Provider + Sync + Send,
{
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

        match provider.get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address).await {
            Ok(current_class_hash) if current_class_hash != contract.diff.local_class_hash => {
                format!("{}: Upgrade", contract.diff.tag)
            }
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                format!("{}: Deploy", contract.diff.tag)
            }
            Ok(_) => "Already Deployed".to_string(),
            Err(_) => format!("{}: Deploy", contract.diff.tag),
        }
    } else {
        format!("{}: Deploy", contract.diff.tag)
    }
}

pub async fn print_strategy<P>(
    ui: &Ui,
    provider: P,
    strategy: &MigrationStrategy,
    world_address: Felt,
) where
    P: Provider + Sync + Send,
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
            ui.print(m.diff.tag.to_string());
            ui.print_sub(format!("Class hash: {:#x}", m.diff.local_class_hash));
        }
    }

    ui.print(" ");

    if !&strategy.contracts.is_empty() {
        ui.print_header(format!("# Contracts ({})", &strategy.contracts.len()));
        for c in &strategy.contracts {
            let op_name = get_contract_operation_name(&provider, c, strategy.world_address).await;

            ui.print(op_name);
            ui.print_sub(format!("Class hash: {:#x}", c.diff.local_class_hash));
            let salt = generate_salt(&get_name_from_tag(&c.diff.tag));
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
    world_address: Felt,
    migration_output: Option<MigrationOutput>,
    salt: &str,
) -> Result<()> {
    let ui = ws.config().ui();
    ui.print_step(5, "✨", "Updating manifests...");

    let deployment_dir = manifest_dir.join(DEPLOYMENT_DIR);

    let deployed_path = deployment_dir.join("manifest").with_extension("toml");
    let deployed_path_json = deployment_dir.join("manifest").with_extension("json");

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
    salt.clone_into(&mut local_manifest.world.inner.seed);

    // when the migration has not been applied because in `plan` mode or because of an error,
    // the `migration_output` is empty.
    if let Some(migration_output) = migration_output {
        // update world deployment transaction hash or block number if they are present in the
        // migration output
        if migration_output.world_tx_hash.is_some() {
            local_manifest.world.inner.transaction_hash = migration_output.world_tx_hash;
        }
        if migration_output.world_block_number.is_some() {
            local_manifest.world.inner.block_number = migration_output.world_block_number;
        }

        migration_output.contracts.iter().for_each(|contract_output| {
            // ignore failed migration which are represented by None
            if let Some(output) = contract_output {
                // find the contract in local manifest and update its base class hash
                let local = local_manifest
                    .contracts
                    .iter_mut()
                    .find(|c| c.inner.tag == output.tag)
                    .expect("contract got migrated, means it should be present here");

                local.inner.base_class_hash = output.base_class_hash;
            }
        });
    }

    // compute contract addresses and update them in the manifest for contracts
    // that have a base class hash set.
    local_manifest.contracts.iter_mut().for_each(|contract| {
        if contract.inner.base_class_hash != Felt::ZERO {
            let salt = generate_salt(&get_name_from_tag(&contract.inner.tag));
            contract.inner.address = Some(get_contract_address(
                salt,
                contract.inner.base_class_hash,
                &[],
                world_address,
            ));
        }
    });

    update_manifest_abis(&mut local_manifest, manifest_dir, profile_name).await;

    local_manifest
        .write_to_path_toml(&deployed_path)
        .with_context(|| "Failed to write toml manifest")?;

    let root_dir = ws.manifest_path().parent().unwrap().to_path_buf();

    local_manifest
        .write_to_path_json(&deployed_path_json, &root_dir)
        .with_context(|| "Failed to write json manifest")?;
    ui.print("\n✨ Done.");

    Ok(())
}

// For now we juust handle writers, handling of owners might be added in the future
pub async fn find_authorization_diff<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    diff: &WorldDiff,
    migration_output: Option<&MigrationOutput>,
    default_namespace: &str,
) -> Result<(Vec<ResourceWriter>, Vec<ResourceWriter>)>
where
    A: ConnectedAccount + Sync + Send,
    <A as Account>::SignError: 'static,
{
    let mut grant = vec![];
    let mut revoke = vec![];

    let mut recently_migrated = HashSet::new();

    if let Some(migration_output) = migration_output {
        recently_migrated = migration_output
            .contracts
            .iter()
            .flatten()
            .map(|m| m.tag.clone())
            .collect::<HashSet<_>>()
    }

    // Generate a map of `Felt` (resource selector) -> `ResourceType` that are available locally
    // so we can check if the resource being revoked is known locally.
    //
    // if the selector is not found in the map we just print its selector
    let resource_map = generate_resource_map(ui, world, diff).await?;

    for c in &diff.contracts {
        // remote is none meants it was not previously deployed.
        // but if it didn't get deployed even during this run we should skip migration for it
        if c.remote_class_hash.is_none() && !recently_migrated.contains(&c.tag) {
            ui.print_sub(format!("Skipping migration for contract {}", c.tag));
            continue;
        }

        let mut local = HashMap::new();
        for write in &c.local_writes {
            let write =
                if write.contains(':') { write.to_string() } else { format!("m:{}", write) };

            let resource = ResourceType::from_str(&write)?;
            let selector = get_resource_selector(ui, world, &resource, default_namespace)
                .await
                .with_context(|| format!("Failed to get selector for {}", write))?;

            let resource_writer = ResourceWriter::from_str(&format!("{},{}", write, c.tag))?;
            local.insert(selector, resource_writer);
        }

        for write in &c.remote_writes {
            // This value is fetched from onchain events, so we get them as felts
            let selector = Felt::from_str(write).with_context(|| "Expected write to be a felt")?;
            if local.remove(&selector).is_some() {
                // do nothing for one which are already onchain
            } else {
                // revoke ones that are not present in local
                assert!(Felt::from_str(write).is_ok());
                revoke.push(ResourceWriter::from_str(&format!("s:{},{}", write, c.tag))?);
            }
        }

        // apply remaining
        local.iter().for_each(|(_, resource_writer)| {
            grant.push(resource_writer.clone());
        });

        let contract_grants: Vec<_> =
            grant.iter().filter(|rw| rw.tag_or_address == c.tag).cloned().collect();
        if !contract_grants.is_empty() {
            ui.print_sub(format!(
                "Granting write access to {} for resources: {:?}",
                c.tag,
                contract_grants
                    .iter()
                    .map(|rw| {
                        let resource = &rw.resource;
                        match resource {
                            // Replace selector with appropriate resource type if present in
                            // resource_map
                            ResourceType::Selector(s) => resource_map
                                .get(&s.to_hex_string())
                                .cloned()
                                .unwrap_or_else(|| rw.resource.clone()),
                            _ => resource.clone(),
                        }
                    })
                    .collect::<Vec<_>>()
            ));
        }

        let contract_revokes: Vec<_> =
            revoke.iter().filter(|rw| rw.tag_or_address == c.tag).cloned().collect();
        if !contract_revokes.is_empty() {
            ui.print_sub(format!(
                "Revoking write access to {} for resources: {:?}",
                c.tag,
                contract_revokes
                    .iter()
                    .map(|rw| {
                        let resource = &rw.resource;
                        match resource {
                            // Replace selector with appropriate resource type if present in
                            // resource_map
                            ResourceType::Selector(s) => resource_map
                                .get(&s.to_hex_string())
                                .cloned()
                                .unwrap_or_else(|| rw.resource.clone()),
                            _ => resource.clone(),
                        }
                    })
                    .collect::<Vec<_>>()
            ));
        }

        if !contract_grants.is_empty() || !contract_revokes.is_empty() {
            ui.print(" ");
        }
    }

    Ok((grant, revoke))
}

// copy abi files from `base/abi` to `deployment/abi` and update abi path in
// local_manifest
async fn update_manifest_abis(
    local_manifest: &mut DeploymentManifest,
    manifest_dir: &Utf8PathBuf,
    profile_name: &str,
) {
    fs::create_dir_all(manifest_dir).await.expect("Failed to create folder");

    async fn inner_helper<T>(
        manifest_dir: &Utf8PathBuf,
        profile_name: &str,
        manifest: &mut Manifest<T>,
    ) where
        T: ManifestMethods,
    {
        let base_relative_path = manifest.inner.abi().unwrap().to_path().unwrap();

        // manifests/dev/base/abis/contract/contract.json -> base/abis/contract/contract.json
        let base_relative_path = base_relative_path
            .strip_prefix(Utf8PathBuf::new().join(MANIFESTS_DIR).join(profile_name))
            .unwrap();

        // base/abis/contract/contract.json -> contract/contract.json
        let stripped_path = base_relative_path
            .strip_prefix(Utf8PathBuf::new().join(BASE_DIR).join(ABIS_DIR))
            .unwrap();

        // deployment/abis/dojo-world.json
        let deployed_relative_path =
            Utf8PathBuf::new().join(DEPLOYMENT_DIR).join(ABIS_DIR).join(stripped_path);

        // <manifest_dir>/base/abis/dojo-world.json
        let full_base_path = manifest_dir.join(base_relative_path);

        // <manifest_dir>/deployment/abis/dojo-world.json
        let full_deployed_path = manifest_dir.join(deployed_relative_path.clone());

        fs::create_dir_all(full_deployed_path.parent().unwrap())
            .await
            .expect("Failed to create folder");

        fs::copy(full_base_path, full_deployed_path).await.expect("Failed to copy abi file");

        manifest.inner.set_abi(Some(AbiFormat::Path(
            Utf8PathBuf::from(MANIFESTS_DIR).join(profile_name).join(deployed_relative_path),
        )));
    }

    inner_helper::<ManifestWorldContract>(manifest_dir, profile_name, &mut local_manifest.world)
        .await;

    inner_helper::<Class>(manifest_dir, profile_name, &mut local_manifest.base).await;

    for contract in local_manifest.contracts.iter_mut() {
        inner_helper::<DojoContract>(manifest_dir, profile_name, contract).await;
    }

    for model in local_manifest.models.iter_mut() {
        inner_helper::<DojoModel>(manifest_dir, profile_name, model).await;
    }

    for event in local_manifest.events.iter_mut() {
        inner_helper::<DojoEvent>(manifest_dir, profile_name, event).await;
    }
}
