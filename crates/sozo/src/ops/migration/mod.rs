use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use dojo_client::contract::world::WorldContract;
use dojo_world::manifest::{Manifest, ManifestError};
use dojo_world::migration::strategy::{prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{Declarable, Deployable, MigrationError, RegisterOutput, StateDiff};
use dojo_world::utils::TransactionWaiter;
use scarb::core::Config;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, FieldElement, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::cairo_short_string_to_felt;
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
use crate::commands::options::world::WorldOptions;
use crate::commands::options::Environment;

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
        setup_env(account, starknet, world, env_metadata.as_ref(), config).await?;

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
        // Prepare migration strategy based on the diff.

        let strategy = prepare_migration(target_dir, diff, name, world_address, config)?;

        println!("  ");

        let block_height = execute_strategy(&strategy, &account, config)
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
    }

    Ok(())
}

async fn setup_env(
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    env_metadata: Option<&Environment>,
    config: &Config,
) -> Result<(Option<FieldElement>, SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>)> {
    let world_address = world.address(env_metadata).ok();

    let account = {
        let provider = starknet.provider(env_metadata)?;
        let mut account = account.account(provider, env_metadata).await?;
        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        let address = account.address();

        config.ui().print(format!("\nMigration account: {address:#x}\n"));

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

// returns the block number at which migration happened
async fn execute_strategy<P, S>(
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ws_config: &Config,
) -> Result<Option<u64>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let mut block_height = None;
    match &strategy.executor {
        Some(executor) => {
            ws_config.ui().print_header("# Executor");

            match executor.deploy(executor.diff.local, vec![], migrator).await {
                Ok(val) => {
                    if let Some(declare) = val.clone().declare {
                        ws_config.ui().print_hidden_sub(format!(
                            "Declare transaction: {:#x}",
                            declare.transaction_hash
                        ));
                    }

                    ws_config.ui().print_hidden_sub(format!(
                        "Deploy transaction: {:#x}",
                        val.transaction_hash
                    ));

                    Ok(())
                }
                Err(MigrationError::ContractAlreadyDeployed) => Ok(()),
                Err(e) => Err(anyhow!("Failed to migrate executor: {:?}", e)),
            }?;

            if strategy.world.is_none() {
                let addr = strategy.world_address()?;
                let InvokeTransactionResult { transaction_hash } =
                    WorldContract::new(addr, &migrator)
                        .set_executor(executor.contract_address)
                        .await?;

                let _ = TransactionWaiter::new(transaction_hash, migrator.provider())
                    .await
                    .map_err(|_| anyhow!("Transaction execution failed"))?;

                ws_config.ui().print_hidden_sub(format!("Updated at: {transaction_hash:#x}"));
            }

            ws_config.ui().print_sub(format!("Contract address: {:#x}", executor.contract_address));
        }
        None => {}
    };

    match &strategy.world {
        Some(world) => {
            ws_config.ui().print_header("# World");

            match world
                .deploy(
                    world.diff.local,
                    vec![strategy.executor.as_ref().unwrap().contract_address],
                    migrator,
                )
                .await
            {
                Ok(val) => {
                    if let Some(declare) = val.clone().declare {
                        ws_config.ui().print_hidden_sub(format!(
                            "Declare transaction: {:#x}",
                            declare.transaction_hash
                        ));
                    }

                    ws_config.ui().print_hidden_sub(format!(
                        "Deploy transaction: {:#x}",
                        val.transaction_hash
                    ));

                    block_height = Some(val.block_number);

                    Ok(())
                }
                Err(MigrationError::ContractAlreadyDeployed) => Err(anyhow!(
                    "Attempting to deploy World at address {:#x} but a World already exists \
                     there. Try using a different World name using `--name`.",
                    world.contract_address
                )),
                Err(e) => Err(anyhow!("Failed to migrate world: {:?}", e)),
            }?;

            ws_config.ui().print_sub(format!("Contract address: {:#x}", world.contract_address));
        }
        None => {}
    };

    register_components(strategy, migrator, ws_config).await?;
    register_systems(strategy, migrator, ws_config).await?;

    Ok(block_height)
}

async fn register_components<P, S>(
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ws_config: &Config,
) -> Result<Option<RegisterOutput>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let components = &strategy.components;

    if components.is_empty() {
        return Ok(None);
    }

    ws_config.ui().print_header(format!("# Components ({})", components.len()));

    let mut declare_output = vec![];

    for c in components.iter() {
        ws_config.ui().print(italic_message(&c.diff.name).to_string());

        let res = c.declare(migrator).await;
        match res {
            Ok(output) => {
                ws_config.ui().print_hidden_sub(format!(
                    "declare transaction: {:#x}",
                    output.transaction_hash
                ));

                declare_output.push(output);
            }

            // Continue if component is already declared
            Err(MigrationError::ClassAlreadyDeclared) => {
                ws_config.ui().print_sub("Already declared");
                continue;
            }
            Err(e) => bail!("Failed to declare component {}: {e}", c.diff.name),
        }

        ws_config.ui().print_sub(format!("class hash: {:#x}", c.diff.local));
    }

    let world_address = strategy.world_address()?;

    let InvokeTransactionResult { transaction_hash } = WorldContract::new(world_address, migrator)
        .register_components(&components.iter().map(|c| c.diff.local).collect::<Vec<_>>())
        .await
        .map_err(|e| anyhow!("Failed to register components to World: {e}"))?;

    let _ = TransactionWaiter::new(transaction_hash, migrator.provider())
        .await
        .map_err(|_| anyhow!("Transaction execution failed"))?;

    ws_config.ui().print_hidden_sub(format!("registered at: {transaction_hash:#x}"));

    Ok(Some(RegisterOutput { transaction_hash, declare_output }))
}

async fn register_systems<P, S>(
    strategy: &MigrationStrategy,
    migrator: &SingleOwnerAccount<P, S>,
    ws_config: &Config,
) -> Result<Option<RegisterOutput>>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let systems = &strategy.systems;

    if systems.is_empty() {
        return Ok(None);
    }

    ws_config.ui().print_header(format!("# Systems ({})", systems.len()));

    let mut declare_output = vec![];

    for s in strategy.systems.iter() {
        ws_config.ui().print(italic_message(&s.diff.name).to_string());

        let res = s.declare(migrator).await;
        match res {
            Ok(output) => {
                ws_config.ui().print_hidden_sub(format!(
                    "Declare transaction: {:#x}",
                    output.transaction_hash
                ));

                declare_output.push(output);
            }

            // Continue if system is already declared
            Err(MigrationError::ClassAlreadyDeclared) => {
                ws_config.ui().print_sub("Already declared");
                continue;
            }
            Err(e) => bail!("Failed to declare system {}: {e}", s.diff.name),
        }

        ws_config.ui().print_sub(format!("class hash: {:#x}", s.diff.local));
    }

    let world_address = strategy.world_address()?;

    let InvokeTransactionResult { transaction_hash } = WorldContract::new(world_address, migrator)
        .register_systems(&systems.iter().map(|s| s.diff.local).collect::<Vec<_>>())
        .await
        .map_err(|e| anyhow!("Failed to register systems to World: {e}"))?;

    let _ = TransactionWaiter::new(transaction_hash, migrator.provider())
        .await
        .map_err(|_| anyhow!("Transaction execution failed"))?;

    ws_config.ui().print_hidden_sub(format!("registered at: {transaction_hash:#x}"));

    Ok(Some(RegisterOutput { transaction_hash, declare_output }))
}
