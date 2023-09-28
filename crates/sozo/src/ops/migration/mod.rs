use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use dojo_world::manifest::{Manifest, ManifestError};
use dojo_world::metadata::Environment;
use dojo_world::migration::contract::ContractMigration;
use dojo_world::migration::strategy::{prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{
    Declarable, DeployOutput, Deployable, MigrationError, RegisterOutput, StateDiff,
};
use dojo_world::utils::TransactionWaiter;
use scarb::core::Config;
use scarb_ui::Ui;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, FieldElement, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use torii_client::contract::world::WorldContract;

#[cfg(test)]
#[path = "migration_test.rs"]
mod migration_test;
mod ui;

use starknet::providers::{
    JsonRpcClient, MaybeUnknownErrorCode, Provider, ProviderError, StarknetErrorWithMessage,
};
use starknet::signers::{LocalWallet, Signer};
use ui::MigrationUi;

use self::ui::{bold_message, italic_message};
use crate::commands::migrate::MigrateArgs;
use crate::commands::options::account::AccountOptions;
use crate::commands::options::starknet::StarknetOptions;
use crate::commands::options::transaction::TransactionOptions;
use crate::commands::options::world::WorldOptions;

pub async fn execute<U>(
    args: MigrateArgs,
    env_metadata: Option<Environment>,
    target_dir: U,
    config: &Config,
) -> Result<()>
where
    U: AsRef<Path>,
{
    let MigrateArgs { account, starknet, world, name, .. } = args;

    // Setup account for migration and fetch world address if it exists.

    let (world_address, account) =
        setup_env(account, starknet, world, env_metadata.as_ref(), config, name.as_ref()).await?;

    // Load local and remote World manifests.

    let (local_manifest, remote_manifest) =
        load_world_manifests(&target_dir, world_address, &account, config).await?;

    // Calculate diff between local and remote World manifests.

    config.ui().print_step(2, "🧰", "Evaluating Worlds diff...");
    let diff = WorldDiff::compute(local_manifest, remote_manifest);
    let total_diffs = diff.count_diffs();
    config.ui().print_sub(format!("Total diffs found: {total_diffs}"));

    if total_diffs == 0 {
        config.ui().print("\n✨ No changes to be made. Remote World is already up to date!")
    } else {
        // Mirate according to the diff.
        apply_diff(target_dir, diff, name, world_address, &account, config, Some(args.transaction))
            .await?;
    }

    Ok(())
}

pub(crate) async fn apply_diff<U, P, S>(
    target_dir: U,
    diff: WorldDiff,
    name: Option<String>,
    world_address: Option<FieldElement>,
    account: &SingleOwnerAccount<P, S>,
    config: &Config,
    txn_config: Option<TransactionOptions>,
) -> Result<FieldElement>
where
    U: AsRef<Path>,
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let strategy = prepare_migration(target_dir, diff, name, world_address, config)?;

    println!("  ");

    let block_height = execute_strategy(&strategy, account, config.ui(), txn_config)
        .await
        .map_err(|e| anyhow!(e))
        .with_context(|| "Problem trying to migrate.")?;

    if let Some(block_height) = block_height {
        config.ui().print(format!(
            "\n🎉 Successfully migrated World on block #{} at address {}",
            block_height,
            bold_message(format!(
                "{:#x}",
                strategy.world_address().expect("world address must exist")
            ))
        ));
    } else {
        config.ui().print(format!(
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
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    env_metadata: Option<&Environment>,
    config: &Config,
    name: Option<&String>,
) -> Result<(Option<FieldElement>, SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>)> {
    let world_address = world.address(env_metadata).ok();

    let account = {
        let provider = starknet.provider(env_metadata)?;
        let mut account = account.account(provider, env_metadata).await?;
        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        let address = account.address();

        config.ui().print(format!("\nMigration account: {address:#x}"));
        if let Some(name) = name {
            config.ui().print(format!("\nWorld name: {name}\n"));
        }

        match account.provider().get_class_hash_at(BlockId::Tag(BlockTag::Pending), address).await {
            Ok(_) => Ok(account),
            Err(ProviderError::StarknetError(StarknetErrorWithMessage {
                code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                ..
            })) => Err(anyhow!("Account with address {:#x} doesn't exist.", account.address())),
            Err(e) => Err(e.into()),
        }
    }
    .with_context(|| "Problem initializing account for migration.")?;

    Ok((world_address, account))
}

async fn load_world_manifests<U, P, S>(
    target_dir: U,
    world_address: Option<FieldElement>,
    account: &SingleOwnerAccount<P, S>,
    config: &Config,
) -> Result<(Manifest, Option<Manifest>)>
where
    U: AsRef<Path>,
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    config.ui().print_step(1, "🌎", "Building World state...");

    let local_manifest = Manifest::load_from_path(target_dir.as_ref().join("manifest.json"))?;

    let remote_manifest = if let Some(world_address) = world_address {
        config.ui().print_sub(format!("Found remote World: {world_address:#x}"));
        config.ui().print_sub("Fetching remote state");

        Manifest::from_remote(account.provider(), world_address, Some(local_manifest.clone()))
            .await
            .map(Some)
            .map_err(|e| match e {
                ManifestError::RemoteWorldNotFound => {
                    anyhow!(
                        "Unable to find remote World at address {world_address:#x}. Make sure the \
                         World address is correct and that it is already deployed!"
                    )
                }
                _ => anyhow!(e),
            })
            .with_context(|| "Failed to build remote World state.")?
    } else {
        config.ui().print_sub("No remote World found");
        None
    };

    Ok((local_manifest, remote_manifest))
}

fn prepare_migration<U>(
    target_dir: U,
    diff: WorldDiff,
    name: Option<String>,
    world_address: Option<FieldElement>,
    config: &Config,
) -> Result<MigrationStrategy>
where
    U: AsRef<Path>,
{
    config.ui().print_step(3, "📦", "Preparing for migration...");

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

    config.ui().print_sub(format!(
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
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ui: &Ui,
    txn_config: Option<TransactionOptions>,
) -> Result<Option<u64>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    match &strategy.executor {
        Some(executor) => {
            ui.print_header("# Executor");
            deploy_contract(executor, "executor", vec![], migrator, ui, &txn_config).await?;

            if strategy.world.is_none() {
                let addr = strategy.world_address()?;
                let InvokeTransactionResult { transaction_hash } =
                    WorldContract::new(addr, &migrator)
                        .set_executor(executor.contract_address)
                        .await?;

                TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

                ui.print_hidden_sub(format!("Updated at: {transaction_hash:#x}"));
            }

            ui.print_sub(format!("Contract address: {:#x}", executor.contract_address));
        }
        None => {}
    };

    match &strategy.world {
        Some(world) => {
            ui.print_header("# World");
            let calldata = vec![strategy.executor.as_ref().unwrap().contract_address];
            deploy_contract(world, "world", calldata, migrator, ui, &txn_config).await?;

            ui.print_sub(format!("Contract address: {:#x}", world.contract_address));
        }
        None => {}
    };

    register_models(strategy, migrator, ui, txn_config.clone()).await?;
    deploy_contracts(strategy, migrator, ui, txn_config).await?;

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
                ui.print_hidden_sub(format!("transaction_hash: {:#x}", output.transaction_hash));

                declare_output.push(output);
            }

            // Continue if model is already declared
            Err(MigrationError::ClassAlreadyDeclared) => {
                ui.print_sub("Already declared");
                continue;
            }
            Err(e) => bail!("Failed to declare model {}: {e}", c.diff.name),
        }

        ui.print_sub(format!("Class hash: {:#x}", c.diff.local));
    }

    let world_address = strategy.world_address()?;

    let InvokeTransactionResult { transaction_hash } = WorldContract::new(world_address, migrator)
        .register_models(&models.iter().map(|c| c.diff.local).collect::<Vec<_>>())
        .await
        .map_err(|e| anyhow!("Failed to register models to World: {e}"))?;

    TransactionWaiter::new(transaction_hash, migrator.provider()).await?;

    ui.print_hidden_sub(format!("registered at: {transaction_hash:#x}"));

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

    for contract in strategy.contracts.iter() {
        let name = &contract.diff.name;
        ui.print(italic_message(name).to_string());
        match deploy_contract(contract, name, vec![], migrator, ui, &txn_config).await? {
            ContractDeploymentOutput::Output(output) => {
                ui.print_sub(format!("Contract address: {:#x}", output.contract_address));
                ui.print_hidden_sub(format!("deploy transaction: {:#x}", output.transaction_hash));
                deploy_output.push(Some(output));
            }
            ContractDeploymentOutput::AlreadyDeployed(contract_address) => {
                ui.print_sub(format!("Already deployed: {:#x}", contract_address));
                deploy_output.push(None);
            }
        }
    }

    Ok(deploy_output)
}
