use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use dojo_world::contracts::cairo_utils;
use dojo_world::contracts::world::WorldContract;
use dojo_world::manifest::{Manifest, ManifestError};
use dojo_world::metadata::dojo_metadata_from_workspace;
use dojo_world::migration::contract::ContractMigration;
use dojo_world::migration::strategy::{generate_salt, prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{
    Declarable, DeployOutput, Deployable, MigrationError, RegisterOutput, StateDiff,
};
use dojo_world::utils::TransactionWaiter;
use scarb::core::Workspace;
use scarb_ui::Ui;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, FieldElement, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::{cairo_short_string_to_felt, get_contract_address};
use starknet::providers::jsonrpc::HttpTransport;

#[cfg(test)]
#[path = "migration_test.rs"]
mod migration_test;
mod ui;

use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use starknet::signers::{LocalWallet, Signer};
use ui::MigrationUi;

use self::ui::{bold_message, italic_message};
use crate::commands::migrate::MigrateArgs;
use crate::commands::options::account::AccountOptions;
use crate::commands::options::starknet::StarknetOptions;
use crate::commands::options::transaction::TransactionOptions;
use crate::commands::options::world::WorldOptions;

pub async fn execute<U>(ws: &Workspace<'_>, args: MigrateArgs, target_dir: U) -> Result<()>
where
    U: AsRef<Path>,
{
    let ui = ws.config().ui();
    let MigrateArgs { account, starknet, world, name, .. } = args;

    // Setup account for migration and fetch world address if it exists.

    let (world_address, account) = setup_env(ws, account, starknet, world, name.as_ref()).await?;

    // Load local and remote World manifests.

    let (local_manifest, remote_manifest) =
        load_world_manifests(&target_dir, &account, world_address, &ui).await?;

    // Calculate diff between local and remote World manifests.

    ui.print_step(2, "🧰", "Evaluating Worlds diff...");
    let diff = WorldDiff::compute(local_manifest.clone(), remote_manifest.clone());
    let total_diffs = diff.count_diffs();
    ui.print_sub(format!("Total diffs found: {total_diffs}"));

    if total_diffs == 0 {
        ui.print("\n✨ No changes to be made. Remote World is already up to date!")
    } else {
        // Mirate according to the diff.
        let world_address = apply_diff(
            ws,
            &target_dir,
            diff,
            name,
            world_address,
            &account,
            Some(args.transaction),
        )
        .await?;

        update_world_manifest(ws, local_manifest, remote_manifest, target_dir, world_address)
            .await?;
    }

    Ok(())
}

async fn update_world_manifest<U>(
    ws: &Workspace<'_>,
    mut local_manifest: Manifest,
    remote_manifest: Option<Manifest>,
    target_dir: U,
    world_address: FieldElement,
) -> Result<()>
where
    U: AsRef<Path>,
{
    let ui = ws.config().ui();
    ui.print("\n✨ Updating manifest.json...");
    local_manifest.world.address = Some(world_address);

    let base_class_hash = match remote_manifest {
        Some(manifest) => manifest.base.class_hash,
        None => local_manifest.base.class_hash,
    };

    local_manifest.contracts.iter_mut().for_each(|c| {
        let salt = generate_salt(&c.name);
        c.address = Some(get_contract_address(salt, base_class_hash, &[], world_address));
    });

    local_manifest.write_to_path(target_dir.as_ref().join("manifest.json"))?;
    ui.print("\n✨ Done.");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_diff<U, P, S>(
    ws: &Workspace<'_>,
    target_dir: U,
    diff: WorldDiff,
    name: Option<String>,
    world_address: Option<FieldElement>,
    account: &SingleOwnerAccount<P, S>,
    txn_config: Option<TransactionOptions>,
) -> Result<FieldElement>
where
    U: AsRef<Path>,
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();
    let strategy = prepare_migration(target_dir, diff, name, world_address, &ui)?;

    println!("  ");

    let block_height = execute_strategy(ws, &strategy, account, txn_config)
        .await
        .map_err(|e| anyhow!(e))
        .with_context(|| "Problem trying to migrate.")?;

    if let Some(block_height) = block_height {
        ui.print(format!(
            "\n🎉 Successfully migrated World on block #{} at address {}",
            block_height,
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

    strategy.world_address()
}

pub(crate) async fn setup_env(
    ws: &Workspace<'_>,
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    name: Option<&String>,
) -> Result<(Option<FieldElement>, SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>)> {
    let ui = ws.config().ui();
    let metadata = dojo_metadata_from_workspace(ws);
    let env = metadata.as_ref().and_then(|inner| inner.env());

    let world_address = world.address(env).ok();

    let account = {
        let provider = starknet.provider(env)?;
        let mut account = account.account(provider, env).await?;
        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        let address = account.address();

        ui.print(format!("\nMigration account: {address:#x}"));
        if let Some(name) = name {
            ui.print(format!("\nWorld name: {name}\n"));
        }

        match account.provider().get_class_hash_at(BlockId::Tag(BlockTag::Pending), address).await {
            Ok(_) => Ok(account),
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                Err(anyhow!("Account with address {:#x} doesn't exist.", account.address()))
            }
            Err(e) => Err(e.into()),
        }
    }
    .with_context(|| "Problem initializing account for migration.")?;

    Ok((world_address, account))
}

async fn load_world_manifests<U, P, S>(
    target_dir: U,
    account: &SingleOwnerAccount<P, S>,
    world_address: Option<FieldElement>,
    ui: &Ui,
) -> Result<(Manifest, Option<Manifest>)>
where
    U: AsRef<Path>,
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    ui.print_step(1, "🌎", "Building World state...");

    let local_manifest = Manifest::load_from_path(target_dir.as_ref().join("manifest.json"))?;

    let remote_manifest = if let Some(address) = world_address {
        match Manifest::load_from_remote(account.provider(), address).await {
            Ok(manifest) => {
                ui.print_sub(format!("Found remote World: {address:#x}"));
                Some(manifest)
            }
            Err(ManifestError::RemoteWorldNotFound) => None,
            Err(e) => return Err(anyhow!("Failed to build remote World state: {e}")),
        }
    } else {
        None
    };

    if remote_manifest.is_none() {
        ui.print_sub("No remote World found");
    }

    Ok((local_manifest, remote_manifest))
}

fn prepare_migration(
    target_dir: impl AsRef<Path>,
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

// returns the Some(block number) at which migration world is deployed, returns none if world was
// not redeployed
pub async fn execute_strategy<P, S>(
    ws: &Workspace<'_>,
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    txn_config: Option<TransactionOptions>,
) -> Result<Option<u64>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let ui = ws.config().ui();

    match &strategy.executor {
        Some(executor) => {
            ui.print_header("# Executor");
            deploy_contract(executor, "executor", vec![], migrator, &ui, &txn_config).await?;

            // There is no world migration, so it exists already.
            if strategy.world.is_none() {
                let addr = strategy.world_address()?;
                let InvokeTransactionResult { transaction_hash } =
                    WorldContract::new(addr, &migrator)
                        .set_executor(&executor.contract_address.into())
                        .send()
                        .await?;

                TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

                ui.print_hidden_sub(format!("Updated at: {transaction_hash:#x}"));
            }

            ui.print_sub(format!("Contract address: {:#x}", executor.contract_address));
        }
        None => {}
    };

    match &strategy.base {
        Some(base) => {
            ui.print_header("# Base Contract");

            match base
                .declare(migrator, txn_config.clone().map(|c| c.into()).unwrap_or_default())
                .await
            {
                Ok(res) => {
                    ui.print_sub(format!("Class Hash: {:#x}", res.class_hash));
                }
                Err(MigrationError::ClassAlreadyDeclared) => {
                    ui.print_sub(format!("Already declared: {:#x}", base.diff.local));
                }
                Err(e) => return Err(e.into()),
            };
        }
        None => {}
    };

    match &strategy.world {
        Some(world) => {
            ui.print_header("# World");

            let calldata = vec![
                strategy.executor.as_ref().unwrap().contract_address,
                strategy.base.as_ref().unwrap().diff.local,
            ];
            deploy_contract(world, "world", calldata.clone(), migrator, &ui, &txn_config)
                .await
                .map_err(|e| anyhow!("Failed to deploy world: {e}"))?;

            ui.print_sub(format!("Contract address: {:#x}", world.contract_address));

            let metadata = dojo_metadata_from_workspace(ws);
            if let Some(meta) = metadata.as_ref().and_then(|inner| inner.world()) {
                match meta.upload().await {
                    Ok(hash) => {
                        let encoded_uri = cairo_utils::encode_uri(&format!("ipfs://{hash}"))?;

                        let InvokeTransactionResult { transaction_hash } =
                            WorldContract::new(world.contract_address, migrator)
                                .set_metadata_uri(&FieldElement::ZERO, &encoded_uri)
                                .send()
                                .await
                                .map_err(|e| anyhow!("Failed to set World metadata: {e}"))?;

                        ui.print_sub(format!("Set Metadata transaction: {:#x}", transaction_hash));
                        ui.print_sub(format!("Metadata uri: ipfs://{hash}"));
                    }
                    Err(err) => {
                        ui.print_sub(format!("Failed to set World metadata:\n{err}"));
                    }
                }
            }
        }
        None => {}
    };

    register_models(strategy, migrator, &ui, txn_config.clone()).await?;
    deploy_contracts(strategy, migrator, &ui, txn_config).await?;

    // This gets current block numder if helpful
    // let block_height = migrator.provider().block_number().await.ok();

    Ok(None)
}

enum ContractDeploymentOutput {
    AlreadyDeployed(FieldElement),
    Output(DeployOutput),
}

async fn deploy_contract<P, S>(
    contract: &ContractMigration,
    contract_id: &str,
    constructor_calldata: Vec<FieldElement>,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: &Option<TransactionOptions>,
) -> Result<ContractDeploymentOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    match contract
        .deploy(
            contract.diff.local,
            constructor_calldata,
            migrator,
            txn_config.clone().map(|c| c.into()).unwrap_or_default(),
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

            ui.print_hidden_sub(format!("Deploy transaction: {:#x}", val.transaction_hash));

            Ok(ContractDeploymentOutput::Output(val))
        }
        Err(MigrationError::ContractAlreadyDeployed(contract_address)) => {
            Ok(ContractDeploymentOutput::AlreadyDeployed(contract_address))
        }
        Err(e) => Err(anyhow!("Failed to migrate {}: {:?}", contract_id, e)),
    }
}

async fn register_models<P, S>(
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: Option<TransactionOptions>,
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

        let res =
            c.declare(migrator, txn_config.clone().map(|c| c.into()).unwrap_or_default()).await;
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
            Err(e) => bail!("Failed to declare model {}: {e}", c.diff.name),
        }

        ui.print_sub(format!("Class hash: {:#x}", c.diff.local));
    }

    let world_address = strategy.world_address()?;
    let world = WorldContract::new(world_address, migrator);

    let calls = models
        .iter()
        .map(|c| world.register_model_getcall(&c.diff.local.into()))
        .collect::<Vec<_>>();

    let InvokeTransactionResult { transaction_hash } = migrator
        .execute(calls)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to register models to World: {e}"))?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print_sub(format!("Registered at: {transaction_hash:#x}"));

    Ok(Some(RegisterOutput { transaction_hash, declare_output }))
}

async fn deploy_contracts<P, S>(
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: Option<TransactionOptions>,
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

    for contract in strategy.contracts.iter() {
        let name = &contract.diff.name;
        ui.print(italic_message(name).to_string());
        match contract
            .world_deploy(
                world_address,
                contract.diff.local,
                migrator,
                txn_config.clone().map(|c| c.into()).unwrap_or_default(),
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

                ui.print_hidden_sub(format!("Deploy transaction: {:#x}", output.transaction_hash));
                ui.print_sub(format!("Contract address: {:#x}", output.contract_address));
                deploy_output.push(Some(output));
            }
            Err(MigrationError::ContractAlreadyDeployed(contract_address)) => {
                ui.print_sub(format!("Already deployed: {:#x}", contract_address));
                deploy_output.push(None);
            }
            Err(e) => return Err(anyhow!("Failed to migrate {}: {:?}", name, e)),
        }
    }

    Ok(deploy_output)
}
