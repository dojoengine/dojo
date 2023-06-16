use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use dojo_world::manifest::{Manifest, ManifestError};
use dojo_world::migration::strategy::{prepare_for_migration, MigrationOutput, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{Declarable, Deployable, MigrationError, RegisterOutput};
use dojo_world::world::WorldContract;
use scarb::core::Config;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement, InvokeTransactionResult};
use starknet::core::utils::cairo_short_string_to_felt;
use yansi::Paint;

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
    ws_config.ui().print(format!("{} 🌏 Building World state...", Paint::new("[1]").dimmed()));

    let local_manifest = Manifest::load_from_path(target_dir.as_ref().join("manifest.json"))?;

    let remote_manifest = if let Some(world_address) = world_address {
        ws_config.ui().print(
            Paint::new(format!(
                "   > Found remote World: {world_address:#x}\n   > Fetching remote state"
            ))
            .dimmed()
            .to_string(),
        );

        Manifest::from_remote(migrator.provider(), world_address, Some(local_manifest.clone()))
            .await
            .map(Some)
            .map_err(|e| match e {
                ManifestError::RemoteWorldNotFound => {
                    anyhow!(
                        "Unable to find remote World at address {world_address:#x}. \
                    Make sure the World address is correct and that it is already deployed!"
                    )
                }
                _ => anyhow!(e),
            })
            .with_context(|| "Failed to build remote World state.")?
    } else {
        ws_config
            .ui()
            .print(Paint::new(format!("   > No remote World found")).dimmed().to_string());
        ws_config.ui().print(
            Paint::new(format!("   > Attempting to deploy a new instance")).dimmed().to_string(),
        );

        None
    };

    ws_config.ui().print(format!("{} 🧰 Evaluating Worlds diff...", Paint::new("[2]").dimmed()));

    let diff = WorldDiff::compute(local_manifest, remote_manifest);

    let total_diffs = diff.count_diffs();

    ws_config
        .ui()
        .print(Paint::new(format!("   > Total diffs found: {}", total_diffs)).dimmed().to_string());

    if total_diffs == 0 {
        ws_config.ui().print("\n✨ No changes to be made. Remote World is already up to date!")
    } else {
        let mut migration = prepare_for_migration(world_address, target_dir, diff)
            .with_context(|| "Problem preparing for migration.")?;

        ws_config.ui().print(format!("{} 📦 Migrating world...", Paint::new("[3]").dimmed()));
        let info = migration.info();
        ws_config.ui().print(
            Paint::new(format!(
                "   > Total items to be migrated ({}): New {} Update {}",
                info.new + info.update,
                info.new,
                info.update
            ))
            .dimmed()
            .to_string(),
        );

        execute_strategy(&mut migration, migrator, ws_config)
            .await
            .map_err(|e| anyhow!(e))
            .with_context(|| "Problem trying to migrate.")?;

        ws_config.ui().print(format!(
            "\n🎉 Successfully migrated World at address {:#x}",
            migration.world_address().expect("world address must exist"),
        ));
    }

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
                .deploy(executor.diff.local, vec![], &migrator)
                .await
                .map_err(|e| anyhow!("Failed to migrate executor: {e}"))?;

            if let Some(declare) = res.clone().declare {
                ws_config.ui().verbose(
                    Paint::new(format!("  > declare transaction: {:#x}", declare.transaction_hash))
                        .dimmed()
                        .to_string(),
                );
            }

            ws_config.ui().verbose(
                Paint::new(format!("  > deploy transaction: {:#x}", res.transaction_hash))
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
                    world.diff.local,
                    vec![strategy.executor.as_ref().unwrap().contract_address.unwrap()],
                    &migrator,
                )
                .await
                .map_err(|e| anyhow!(e))?;

            if let Some(declare) = res.clone().declare {
                ws_config.ui().verbose(
                    Paint::new(format!("  > declare transaction: {:#x}", declare.transaction_hash))
                        .dimmed()
                        .to_string(),
                );
            }

            ws_config.ui().verbose(
                Paint::new(format!("  > deploy transaction: {:#x}", res.transaction_hash))
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

    if world_output.is_some() {
        configure_admin(strategy, &migrator, ws_config).await?;
    }

    Ok(MigrationOutput {
        world: world_output,
        executor: executor_output,
        systems: systems_output,
        components: components_output,
    })
}

async fn configure_admin<A>(
    strategy: &MigrationStrategy,
    migrator: &A,
    ws_config: &Config,
) -> Result<()>
where
    A: ConnectedAccount + Sync + 'static,
{
    ws_config.ui().print(Paint::new("# Initialization").bold().to_string());

    ws_config.ui().verbose(format!("  Configuring Admin role for {:#x}", &migrator.address()));

    let world_address = strategy.world_address()?;
    let res = WorldContract::new(world_address, migrator)
        .system("GrantAuthRole", BlockId::Tag(BlockTag::Latest))
        .await?
        .execute(vec![migrator.address(), cairo_short_string_to_felt("sudo")?])
        .await
        .unwrap();

    ws_config.ui().verbose(
        Paint::new(format!("  > transaction: {:#x}", res.transaction_hash)).dimmed().to_string(),
    );

    Ok(())
}

async fn register_components<A>(
    strategy: &MigrationStrategy,
    migrator: &A,
    ws_config: &Config,
) -> Result<Option<RegisterOutput>>
where
    A: ConnectedAccount + Sync + 'static,
{
    if strategy.components.is_empty() {
        return Ok(None);
    }

    ws_config.ui().print(
        Paint::new(format!("# Components ({})", strategy.components.len())).bold().to_string(),
    );

    let mut declare_output = vec![];

    for component in strategy.components.iter() {
        ws_config.ui().print(format!("  {}", Paint::new(&component.diff.name).italic()));

        let res = component
            .declare(migrator)
            .await
            .map_err(|e| anyhow!("Failed to declare component {}: {e}", component.diff.name))?;

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

    Ok(Some(RegisterOutput { transaction_hash, declare_output }))
}

async fn register_systems<A>(
    strategy: &MigrationStrategy,
    migrator: &A,
    ws_config: &Config,
) -> Result<Option<RegisterOutput>>
where
    A: ConnectedAccount + Sync + 'static,
{
    if strategy.systems.is_empty() {
        return Ok(None);
    }

    ws_config
        .ui()
        .print(Paint::new(format!("# Systems ({})", strategy.systems.len())).bold().to_string());

    let mut declare_output = vec![];

    for system in strategy.systems.iter() {
        ws_config.ui().print(format!("  {}", Paint::new(&system.diff.name).italic()));

        let res = system
            .declare(migrator)
            .await
            .map_err(|e| anyhow!("Failed to declare system {}: {e}", system.diff.name))?;

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

    Ok(Some(RegisterOutput { transaction_hash, declare_output }))
}
