use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use dojo_client::contract::world::WorldContract;
use dojo_world::manifest::{Manifest, ManifestError};
use dojo_world::migration::strategy::{prepare_for_migration, MigrationOutput, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{Declarable, Deployable, MigrationError, RegisterOutput};
use dojo_world::utils::TransactionWaiter;
use scarb::core::Config;
use starknet::accounts::{ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{FieldElement, InvokeTransactionResult};

#[cfg(test)]
#[path = "migration_test.rs"]
mod migration_test;
mod ui;

use starknet::providers::Provider;
use starknet::signers::Signer;
use ui::MigrationUi;

use self::ui::{bold_message, italic_message};

pub async fn execute<U, P, S>(
    world_address: Option<FieldElement>,
    migrator: SingleOwnerAccount<P, S>,
    target_dir: U,
    ws_config: &Config,
) -> Result<()>
where
    U: AsRef<Path>,
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    ws_config.ui().print_step(1, "🌎", "Building World state...");

    let local_manifest = Manifest::load_from_path(target_dir.as_ref().join("manifest.json"))?;

    let remote_manifest = if let Some(world_address) = world_address {
        ws_config.ui().print_sub(format!("Found remote World: {world_address:#x}"));
        ws_config.ui().print_sub("Fetching remote state");

        Manifest::from_remote(migrator.provider(), world_address, Some(local_manifest.clone()))
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
        ws_config.ui().print_sub("No remote World found");
        None
    };

    ws_config.ui().print_step(2, "🧰", "Evaluating Worlds diff...");

    let diff = WorldDiff::compute(local_manifest, remote_manifest);

    let total_diffs = diff.count_diffs();

    ws_config.ui().print_sub(format!("Total diffs found: {total_diffs}"));

    if total_diffs == 0 {
        ws_config.ui().print("\n✨ No changes to be made. Remote World is already up to date!")
    } else {
        ws_config.ui().print_step(3, "📦", "Preparing for migration...");

        let mut migration = prepare_for_migration(world_address, target_dir, diff)
            .with_context(|| "Problem preparing for migration.")?;

        let info = migration.info();

        ws_config.ui().print_sub(format!(
            "Total items to be migrated ({}): New {} Update {}",
            info.new + info.update,
            info.new,
            info.update
        ));

        println!("  ");

        execute_strategy(&mut migration, migrator, ws_config)
            .await
            .map_err(|e| anyhow!(e))
            .with_context(|| "Problem trying to migrate.")?;

        ws_config.ui().print(format!(
            "\n🎉 Successfully migrated World at address {}",
            bold_message(format!(
                "{:#x}",
                migration.world_address().expect("world address must exist")
            ))
        ));
    }

    Ok(())
}

// TODO: display migration type (either new or update)
async fn execute_strategy<P, S>(
    strategy: &mut MigrationStrategy,
    migrator: SingleOwnerAccount<P, S>,
    ws_config: &Config,
) -> Result<MigrationOutput>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let executor_output = match &mut strategy.executor {
        Some(executor) => {
            ws_config.ui().print_header("# Executor");

            let res = executor
                .deploy(executor.diff.local, vec![], &migrator)
                .await
                .map_err(|e| anyhow!("Failed to migrate executor: {e}"))?;

            if let Some(declare) = res.clone().declare {
                ws_config.ui().print_hidden_sub(format!(
                    "declare transaction: {:#x}",
                    declare.transaction_hash
                ));
            }

            ws_config
                .ui()
                .print_hidden_sub(format!("deploy transaction: {:#x}", res.transaction_hash));

            if strategy.world.is_none() {
                let addr = strategy.world_address()?;
                let InvokeTransactionResult { transaction_hash } =
                    WorldContract::new(addr, &migrator).set_executor(res.contract_address).await?;

                let _ = TransactionWaiter::new(transaction_hash, migrator.provider())
                    .await
                    .map_err(MigrationError::<S, <P as Provider>::Error>::WaitingError);

                ws_config.ui().print_hidden_sub(format!("updated at: {transaction_hash:#x}"));
            }

            ws_config.ui().print_sub(format!("contract address: {:#x}", res.contract_address));

            Some(res)
        }
        None => None,
    };

    let world_output = match &mut strategy.world {
        Some(world) => {
            ws_config.ui().print_header("# World");

            let res = world
                .deploy(
                    world.diff.local,
                    vec![strategy.executor.as_ref().unwrap().contract_address.unwrap()],
                    &migrator,
                )
                .await
                .map_err(|e| anyhow!(e))?;

            if let Some(declare) = res.clone().declare {
                ws_config.ui().print_hidden_sub(format!(
                    "declare transaction: {:#x}",
                    declare.transaction_hash
                ));
            }

            ws_config
                .ui()
                .print_hidden_sub(format!("deploy transaction: {:#x}", res.transaction_hash));

            ws_config.ui().print_sub(format!("contract address: {:#x}", res.contract_address));

            Some(res)
        }
        None => None,
    };

    let components_output = register_components(strategy, &migrator, ws_config).await?;
    let systems_output = register_systems(strategy, &migrator, ws_config).await?;

    Ok(MigrationOutput {
        world: world_output,
        executor: executor_output,
        systems: systems_output,
        components: components_output,
    })
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
                ws_config.ui().print_sub("already declared");
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
        .map_err(MigrationError::<S, <P as Provider>::Error>::WaitingError);

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
                    "declare transaction: {:#x}",
                    output.transaction_hash
                ));

                declare_output.push(output);
            }

            // Continue if system is already declared
            Err(MigrationError::ClassAlreadyDeclared) => {
                ws_config.ui().print_sub("already declared");
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
        .map_err(MigrationError::<S, <P as Provider>::Error>::WaitingError);

    ws_config.ui().print_hidden_sub(format!("registered at: {transaction_hash:#x}"));

    Ok(Some(RegisterOutput { transaction_hash, declare_output }))
}
