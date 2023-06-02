use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use dojo_world::config::WorldConfig;
use dojo_world::migration::object::{
    ClassMigration, ContractMigration, Declarable, DeployOutput, Deployable, MigrationError,
    RegisterOutput, WorldContract,
};
use dojo_world::migration::world::{ClassDiff, ContractDiff, WorldDiff};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{FieldElement, InvokeTransactionResult};
use starknet::providers::Provider;

pub type MigrationResult<S, P> = Result<MigrationOutput, MigrationError<S, P>>;

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

impl MigrationStrategy {
    pub async fn execute<A>(
        &mut self,
        migrator: A,
    ) -> MigrationResult<A::SignError, <A::Provider as Provider>::Error>
    where
        A: ConnectedAccount + Sync,
    {
        let executor_output = match &mut self.executor {
            Some(executor) => {
                let res = executor.deploy(vec![], &migrator).await?;

                if self.world.is_none() {
                    let addr = self.world_address().ok_or(MigrationError::WorldAddressNotFound)?;
                    WorldContract::new(addr, &migrator).set_executor(res.contract_address).await?;
                }

                Some(res)
            }
            None => None,
        };

        let world_output = match &mut self.world {
            Some(world) => world
                .deploy(vec![self.executor.as_ref().unwrap().contract_address.unwrap()], &migrator)
                .await
                .map(|o| Some(o))?,
            None => None,
        };

        let components_output = self.register_systems(&migrator).await?;
        let systems_output = self.register_components(&migrator).await?;

        Ok(MigrationOutput {
            world: world_output,
            executor: executor_output,
            systems: systems_output,
            components: components_output,
        })
    }

    async fn register_components<A>(
        &self,
        migrator: &A,
    ) -> Result<RegisterOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
    where
        A: ConnectedAccount + Sync,
    {
        let mut declare_output = vec![];
        for component in &self.components {
            declare_output.push(component.declare(migrator).await?);
        }

        let world_address = self.world_address().ok_or(MigrationError::WorldAddressNotFound)?;

        let InvokeTransactionResult { transaction_hash } =
            WorldContract::new(world_address, migrator)
                .register_components(
                    &declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>(),
                )
                .await?;

        Ok(RegisterOutput { transaction_hash, declare_output })
    }

    async fn register_systems<A>(
        &self,
        migrator: &A,
    ) -> Result<RegisterOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
    where
        A: ConnectedAccount + Sync,
    {
        let mut declare_output = vec![];
        for system in &self.systems {
            declare_output.push(system.declare(migrator).await?);
        }

        let world_address = self.world_address().ok_or(MigrationError::WorldAddressNotFound)?;

        let InvokeTransactionResult { transaction_hash } =
            WorldContract::new(world_address, migrator)
                .register_components(
                    &declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>(),
                )
                .await?;

        Ok(RegisterOutput { transaction_hash, declare_output })
    }
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
                syst_to_migrate.push(ClassMigration {
                    // declared: false,
                    class: s.clone(),
                    artifact_path: path.clone(),
                });
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
                comps_to_migrate.push(ClassMigration {
                    class: c.clone(),
                    artifact_path: path.clone(),
                    ..Default::default()
                });
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
