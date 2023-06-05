use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use scarb::core::Config;
use scarb::ui::Ui;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag, StarknetError};
use starknet::providers::{Provider, ProviderError};
use starknet::{accounts::ConnectedAccount, core::types::InvokeTransactionResult};

pub mod config;
pub mod object;
pub mod strategy;
pub mod world;

#[cfg(test)]
#[path = "migration_test.rs"]
mod migration_test;

use object::{Declarable, Deployable, MigrationError, RegisterOutput, WorldContract};
use strategy::{MigrationOutput, MigrationResult, MigrationStrategy};

use self::config::{EnvironmentConfig, WorldConfig};
use self::strategy::prepare_for_migration;
use self::world::WorldDiff;

pub async fn execute(
    world_config: WorldConfig,
    environment_config: EnvironmentConfig,
    ws_config: &Config,
) -> Result<()> {
    let migrator = environment_config
        .migrator()
        .await
        .with_context(|| "Failed to initialize migrator account")?;

    migrator
        .provider()
        .get_class_hash_at(BlockId::Tag(BlockTag::Pending), migrator.address())
        .await
        .map_err(|e| match e {
            ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                anyhow!("Migrator account doesn't exist: {:#x}", migrator.address())
            }
            _ => anyhow!(e),
        })?;

    ws_config.ui().print("🔍 Building world state...");

    let target_dir = ws_config.target_dir().path_existent()?.to_path_buf();

    let diff = WorldDiff::from_path(target_dir.clone(), &world_config, &environment_config).await?;
    let mut migration = prepare_for_migration(target_dir, diff, world_config)?;

    ws_config.ui().print("🌎 Migrating world...");

    let output = execute_strategy(&mut migration, migrator, &ws_config)
        .await
        .map_err(|e| anyhow!(e))
        .with_context(|| "Failed to migrate")?;

    ws_config.ui().print(format!(
        "\n✨ Successfully migrated world at address {:#x}",
        output
            .world
            .as_ref()
            .map(|o| o.contract_address)
            .or(world_config.address)
            .expect("world address must exist"),
    ));

    Ok(())
}

async fn execute_strategy<A>(
    strategy: &mut MigrationStrategy,
    migrator: A,
    ws_config: &Config,
) -> MigrationResult<A::SignError, <A::Provider as Provider>::Error>
where
    A: ConnectedAccount + Sync,
{
    let multi_progress = MultiProgress::new();

    let executor_output = match &mut strategy.executor {
        Some(executor) => {
            let eb = ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("{spinner} executor: {msg}").unwrap());
            eb.enable_steady_tick(Duration::from_millis(100));

            multi_progress.add(eb.clone());

            eb.set_message("deploying contract");
            let res = executor.deploy(vec![], &migrator).await?;
            eb.finish_with_message("deployed");

            if strategy.world.is_none() {
                let addr = strategy.world_address().ok_or(MigrationError::WorldAddressNotFound)?;
                WorldContract::new(addr, &migrator).set_executor(res.contract_address).await?;
            }

            Some(res)
        }
        None => None,
    };

    let world_output = match &mut strategy.world {
        Some(world) => {
            let wb = ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("{spinner} world: {msg}").unwrap());
            wb.enable_steady_tick(Duration::from_millis(100));
            multi_progress.add(wb.clone());

            wb.set_message("deploying contract");

            let res = world
                .deploy(
                    vec![strategy.executor.as_ref().unwrap().contract_address.unwrap()],
                    &migrator,
                )
                .await?;

            wb.finish_with_message("deployed");

            Some(res)
        }
        None => None,
    };

    let components_output = register_components(&strategy, &migrator, &multi_progress).await?;
    let systems_output = register_systems(&strategy, &migrator, &multi_progress).await?;

    multi_progress.clear().expect("should be able to clear progress bar");

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
    multi_progress: &MultiProgress,
) -> Result<RegisterOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
where
    A: ConnectedAccount + Sync,
{
    let cb = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template("{spinner} components: {msg}").unwrap());
    cb.enable_steady_tick(Duration::from_millis(100));

    multi_progress.add(cb.clone());

    let total = strategy.components.len();
    let mut declare_output = vec![];

    for (i, component) in strategy.components.iter().enumerate() {
        cb.set_message(format!("({i}/{total}) declaring {} class", component.class.name));
        let res = component.declare(migrator).await?;
        declare_output.push(res);
    }

    let world_address = strategy.world_address().ok_or(MigrationError::WorldAddressNotFound)?;

    cb.set_message(format!("registering components to world"));
    let InvokeTransactionResult { transaction_hash } = WorldContract::new(world_address, migrator)
        .register_components(&declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>())
        .await?;

    cb.set_message("registered");
    cb.finish();

    Ok(RegisterOutput { transaction_hash, declare_output })
}

async fn register_systems<A>(
    strategy: &MigrationStrategy,
    migrator: &A,
    multi_progress: &MultiProgress,
) -> Result<RegisterOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
where
    A: ConnectedAccount + Sync,
{
    let sb = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template("{spinner} systems: {msg}").unwrap());
    sb.enable_steady_tick(Duration::from_millis(100));

    multi_progress.add(sb.clone());

    let total = strategy.systems.len();
    let mut declare_output = vec![];

    for (i, system) in strategy.systems.iter().enumerate() {
        sb.set_message(format!("({i}/{total}) declaring {} class", system.class.name));
        let res = system.declare(migrator).await?;
        declare_output.push(res);
    }

    let world_address = strategy.world_address().ok_or(MigrationError::WorldAddressNotFound)?;

    sb.set_message("registering systems to world...");

    let InvokeTransactionResult { transaction_hash } = WorldContract::new(world_address, migrator)
        .register_systems(&declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>())
        .await?;

    sb.finish_with_message("registered");

    Ok(RegisterOutput { transaction_hash, declare_output })
}
