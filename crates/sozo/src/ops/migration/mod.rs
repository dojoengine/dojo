use std::path::Path;

use anyhow::{anyhow, Context, Result};
use dojo_world::manifest::Manifest;
use dojo_world::migration::strategy::{prepare_for_migration, MigrationOutput, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{Declarable, Deployable, RegisterOutput};
use dojo_world::world::WorldContract;
use scarb::core::Config;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{FieldElement, InvokeTransactionResult};
use yansi::Paint;

pub mod config;

#[cfg(test)]
#[path = "migration_test.rs"]
mod migration_test;

pub async fn execute<P, A>(
    world_address: Option<FieldElement>,
    migrator: A,
    target_dir: P,
    ws_config: &Config,
) -> Result<()>
where
    P: AsRef<Path>,
    A: ConnectedAccount + Sync + 'static,
{
    ws_config.ui().print(format!("{} 🌏 Building World state...", Paint::new("[1/3]").dimmed()));

    let local_manifest = Manifest::load_from_path(target_dir.as_ref().join("manifest.json"))?;

    let remote_manifest = if let Some(world_address) = world_address {
        ws_config.ui().print(
            Paint::new(format!(
                "   > Found remote World: {world_address:#x}\n   > Fetching remote World state"
            ))
            .dimmed()
            .to_string(),
        );

        Manifest::from_remote(migrator.provider(), world_address, Some(local_manifest.clone()))
            .await
            .map(Some)
            .map_err(|e| anyhow!("Failed creating remote World manifest: {e}"))?
    } else {
        None
    };

    let diff = WorldDiff::compute(local_manifest, remote_manifest);

    ws_config.ui().print(format!("{} 🧰 Evaluating World diff...", Paint::new("[2/3]").dimmed()));

    let mut migration = prepare_for_migration(world_address, target_dir, diff)
        .with_context(|| "Problem preparing for migration.")?;

    ws_config.ui().print(format!("{} 📦 Migrating world...", Paint::new("[3/3]").dimmed()));

    let output = execute_strategy(&mut migration, migrator, ws_config)
        .await
        .map_err(|e| anyhow!(e))
        .with_context(|| "Problem trying to migrate.")?;

    ws_config.ui().print(format!(
        "\n✨ Successfully migrated World at address {:#x}",
        output
            .world
            .as_ref()
            .map(|o| o.contract_address)
            .or(world_address)
            .expect("world address must exist"),
    ));

    Ok(())
}

// TODO: display migration type (either new or update)
async fn execute_strategy<A>(
    strategy: &mut MigrationStrategy,
    migrator: A,
    ws_config: &Config,
) -> Result<MigrationOutput>
where
    A: ConnectedAccount + Sync + 'static,
{
    let executor_output = match &mut strategy.executor {
        Some(executor) => {
            ws_config.ui().print(format!("\n{}", Paint::new("# Executor").bold()));

            let res = executor
                .deploy(vec![], &migrator)
                .await
                .map_err(|e| anyhow!("Failed to migrate executor: {e}"))?;

            ws_config.ui().verbose(
                Paint::new(format!(
                    "  > declare transaction: {:#x}\n  > deploy transaction: {:#x}",
                    res.declare_res.transaction_hash, res.transaction_hash
                ))
                .dimmed()
                .to_string(),
            );

            if strategy.world.is_none() {
                let addr = strategy.world_address()?;
                let InvokeTransactionResult { transaction_hash } =
                    WorldContract::new(addr, &migrator).set_executor(res.contract_address).await?;

                ws_config.ui().verbose(
                    Paint::new(format!("  > updated at: {:#x}", transaction_hash))
                        .dimmed()
                        .to_string(),
                );
            }

            ws_config.ui().print(
                Paint::new(format!("  > contract address: {:#x}", res.contract_address))
                    .dimmed()
                    .to_string(),
            );

            Some(res)
        }
        None => None,
    };

    let world_output = match &mut strategy.world {
        Some(world) => {
            ws_config.ui().print(Paint::new("# World").bold().to_string());

            let res = world
                .deploy(
                    vec![strategy.executor.as_ref().unwrap().contract_address.unwrap()],
                    &migrator,
                )
                .await
                .map_err(|e| anyhow!(e))?;

            ws_config.ui().verbose(
                Paint::new(format!(
                    "  > declare transaction: {:#x}\n  > deploy transaction: {:#x}",
                    res.declare_res.transaction_hash, res.transaction_hash
                ))
                .dimmed()
                .to_string(),
            );

            ws_config.ui().print(
                Paint::new(format!("  > contract address: {:#x}", res.contract_address))
                    .dimmed()
                    .to_string(),
            );

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

async fn register_components<A>(
    strategy: &MigrationStrategy,
    migrator: &A,
    ws_config: &Config,
) -> Result<RegisterOutput>
where
    A: ConnectedAccount + Sync + 'static,
{
    ws_config.ui().print(
        Paint::new(format!("# Components ({})", strategy.components.len())).bold().to_string(),
    );

    let mut declare_output = vec![];

    for component in strategy.components.iter() {
        ws_config.ui().print(format!("  {}", Paint::new(&component.class.name).italic()));

        let res = component
            .declare(migrator)
            .await
            .map_err(|e| anyhow!("Failed to declare component {}: {e}", component.class.name))?;

        ws_config.ui().verbose(
            Paint::new(format!("  > declare transaction: {:#x}", res.transaction_hash))
                .dimmed()
                .to_string(),
        );

        ws_config.ui().print(
            Paint::new(format!("  > class hash: {:#x}", res.class_hash)).dimmed().to_string(),
        );

        declare_output.push(res);
    }

    let world_address = strategy.world_address()?;

    let InvokeTransactionResult { transaction_hash } = WorldContract::new(world_address, migrator)
        .register_components(&declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>())
        .await
        .map_err(|e| anyhow!("Failed to register components to World {world_address:#x}: {e}"))?;

    ws_config.ui().verbose(
        Paint::new(format!("  > registered at: {:#x}", transaction_hash)).dimmed().to_string(),
    );

    Ok(RegisterOutput { transaction_hash, declare_output })
}

async fn register_systems<A>(
    strategy: &MigrationStrategy,
    migrator: &A,
    ws_config: &Config,
) -> Result<RegisterOutput>
where
    A: ConnectedAccount + Sync + 'static,
{
    ws_config
        .ui()
        .print(Paint::new(format!("# Systems ({})", strategy.systems.len())).bold().to_string());

    let mut declare_output = vec![];

    for system in strategy.systems.iter() {
        ws_config.ui().print(format!("  {}", Paint::new(&system.class.name).italic()));

        let res = system
            .declare(migrator)
            .await
            .map_err(|e| anyhow!("Failed to declare system {}: {e}", system.class.name))?;

        ws_config.ui().verbose(
            Paint::new(format!("  > declare transaction: {:#x}", res.transaction_hash))
                .dimmed()
                .to_string(),
        );

        ws_config.ui().print(
            Paint::new(format!("  > class hash: {:#x}", res.class_hash)).dimmed().to_string(),
        );

        declare_output.push(res);
    }

    let world_address = strategy.world_address()?;

    let InvokeTransactionResult { transaction_hash } = WorldContract::new(world_address, migrator)
        .register_systems(&declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>())
        .await
        .map_err(|e| anyhow!("Failed to register systems to World {world_address:#x}: {e}"))?;

    ws_config.ui().verbose(
        Paint::new(format!("  > registered at: {:#x}", transaction_hash)).dimmed().to_string(),
    );

    Ok(RegisterOutput { transaction_hash, declare_output })
}
