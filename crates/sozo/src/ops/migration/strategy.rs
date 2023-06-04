use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{FieldElement, InvokeTransactionResult};
use starknet::providers::Provider;

use super::config::WorldConfig;
use super::object::{
    ClassMigration, ContractMigration, Declarable, DeployOutput, Deployable, MigrationError,
    RegisterOutput, WorldContract,
};
use super::world::{ClassDiff, ContractDiff, WorldDiff};

pub type MigrationResult<S, P> = Result<MigrationOutput, MigrationError<S, P>>;

#[cfg(test)]
#[path = "strategy_test.rs"]
mod test;

#[derive(Debug)]
pub struct MigrationOutput {
    pub world: Option<DeployOutput>,
    pub executor: Option<DeployOutput>,
    pub systems: RegisterOutput,
    pub components: RegisterOutput,
}

#[derive(Debug)]
pub struct MigrationStrategy {
    pub world: Option<ContractMigration>,
    pub executor: Option<ContractMigration>,
    pub systems: Vec<ClassMigration>,
    pub components: Vec<ClassMigration>,
    pub world_config: WorldConfig,
}

impl MigrationStrategy {
    fn world_address(&self) -> Option<FieldElement> {
        match &self.world {
            Some(c) => c.contract_address,
            None => self.world_config.address,
        }
    }
}

pub async fn execute_migration<A>(
    strategy: &mut MigrationStrategy,
    migrator: A,
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

/// construct migration strategy
/// evaluate which contracts/classes need to be declared/deployed
pub fn prepare_for_migration(
    target_dir: Utf8PathBuf,
    diff: WorldDiff,
    world_config: WorldConfig,
) -> Result<MigrationStrategy> {
    let entries = fs::read_dir(target_dir)
        .map_err(|err| anyhow!("Failed reading source directory: {err}"))?;

    let mut artifact_paths = HashMap::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        if file_name_str == "manifest.json" || !file_name_str.ends_with(".json") {
            continue;
        }

        let name = file_name_str.split('_').last().unwrap().trim_end_matches(".json").to_string();

        artifact_paths.insert(name, entry.path());
    }

    // We don't need to care if a contract has already been declared or not, because
    // the migration strategy will take care of that.

    // If the world contract needs to be migrated, then all contracts need to be migrated
    // else we need to evaluate which contracts need to be migrated.
    let world = evaluate_contract_to_migrate(&diff.world, &artifact_paths, false)?;
    let executor = evaluate_contract_to_migrate(&diff.executor, &artifact_paths, world.is_some())?;
    let components =
        evaluate_components_to_migrate(&diff.components, &artifact_paths, world.is_some())?;
    let systems = evaluate_systems_to_migrate(&diff.systems, &artifact_paths, world.is_some())?;

    Ok(MigrationStrategy { world, executor, systems, components, world_config })
}

fn evaluate_systems_to_migrate(
    systems: &[ClassDiff],
    artifact_paths: &HashMap<String, PathBuf>,
    world_contract_will_migrate: bool,
) -> Result<Vec<ClassMigration>> {
    let mut syst_to_migrate = vec![];

    for s in systems {
        match s.remote {
            Some(remote) if remote == s.local && !world_contract_will_migrate => continue,
            _ => {
                let path = find_artifact_path(&format!("{}System", s.name), artifact_paths)?;
                syst_to_migrate
                    .push(ClassMigration { class: s.clone(), artifact_path: path.clone() });
            }
        }
    }

    Ok(syst_to_migrate)
}

fn evaluate_components_to_migrate(
    components: &[ClassDiff],
    artifact_paths: &HashMap<String, PathBuf>,
    world_contract_will_migrate: bool,
) -> Result<Vec<ClassMigration>> {
    let mut comps_to_migrate = vec![];

    for c in components {
        match c.remote {
            Some(remote) if remote == c.local && !world_contract_will_migrate => continue,
            _ => {
                let path = find_artifact_path(&format!("{}Component", c.name), artifact_paths)?;
                comps_to_migrate
                    .push(ClassMigration { class: c.clone(), artifact_path: path.clone() });
            }
        }
    }

    Ok(comps_to_migrate)
}

fn evaluate_contract_to_migrate(
    contract: &ContractDiff,
    artifact_paths: &HashMap<String, PathBuf>,
    world_contract_will_migrate: bool,
) -> Result<Option<ContractMigration>> {
    if world_contract_will_migrate
        || contract.address.is_none()
        || matches!(contract.remote, Some(remote_hash) if remote_hash != contract.local)
    {
        let path = find_artifact_path(&contract.name, artifact_paths)?;

        // TODO: generate random salt
        Ok(Some(ContractMigration {
            contract: contract.clone(),
            artifact_path: path.clone(),
            ..Default::default()
        }))
    } else {
        Ok(None)
    }
}

fn find_artifact_path<'a>(
    contract_name: &str,
    artifact_paths: &'a HashMap<String, PathBuf>,
) -> Result<&'a PathBuf> {
    artifact_paths
        .get(contract_name)
        .with_context(|| anyhow!("missing contract artifact for `{}` contract", contract_name))
}
