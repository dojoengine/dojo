use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use convert_case::{Case, Casing};
use starknet::core::types::FieldElement;

use super::class::{ClassDiff, ClassMigration};
use super::contract::{ContractDiff, ContractMigration};
use super::world::WorldDiff;
use super::{DeployOutput, MigrationType, RegisterOutput};

#[derive(Debug)]
pub struct MigrationOutput {
    pub world: Option<DeployOutput>,
    pub executor: Option<DeployOutput>,
    pub systems: Option<RegisterOutput>,
    pub components: Option<RegisterOutput>,
}

#[derive(Debug)]
pub struct MigrationStrategy {
    pub world_address: Option<FieldElement>,
    pub world: Option<ContractMigration>,
    pub executor: Option<ContractMigration>,
    pub systems: Vec<ClassMigration>,
    pub components: Vec<ClassMigration>,
}

#[derive(Debug)]
pub struct MigrationItemsInfo {
    pub new: usize,
    pub update: usize,
}

impl MigrationStrategy {
    pub fn world_address(&self) -> Result<FieldElement> {
        let addr = match &self.world {
            Some(c) => c.contract_address,
            None => self.world_address,
        };

        addr.ok_or_else(|| anyhow!("World address not found"))
    }

    pub fn info(&self) -> MigrationItemsInfo {
        let mut new = 0;
        let mut update = 0;

        if let Some(item) = &self.world {
            match item.migration_type() {
                MigrationType::New => new += 1,
                MigrationType::Update => update += 1,
            }
        }

        if let Some(item) = &self.executor {
            match item.migration_type() {
                MigrationType::New => new += 1,
                MigrationType::Update => update += 1,
            }
        }

        self.systems.iter().for_each(|item| match item.migration_type() {
            MigrationType::New => new += 1,
            MigrationType::Update => update += 1,
        });

        self.components.iter().for_each(|item| match item.migration_type() {
            MigrationType::New => new += 1,
            MigrationType::Update => update += 1,
        });

        MigrationItemsInfo { new, update }
    }
}

/// construct migration strategy
/// evaluate which contracts/classes need to be declared/deployed
pub fn prepare_for_migration<P>(
    world_address: Option<FieldElement>,
    target_dir: P,
    diff: WorldDiff,
) -> Result<MigrationStrategy>
where
    P: AsRef<Path>,
{
    let entries = fs::read_dir(target_dir)
        .map_err(|err| anyhow!("Failed reading source directory: {err}"))?;

    let mut artifact_paths = HashMap::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        if file_name_str == "manifest.json" || !file_name_str.ends_with(".json") {
            continue;
        }

        let name = file_name_str.split('-').last().unwrap().trim_end_matches(".json").to_string();

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

    Ok(MigrationStrategy { world_address, world, executor, systems, components })
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
                let path = find_artifact_path(&s.name, artifact_paths)?;
                syst_to_migrate
                    .push(ClassMigration { diff: s.clone(), artifact_path: path.clone() });
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
                let path =
                    find_artifact_path(c.name.to_case(Case::Snake).as_str(), artifact_paths)?;
                comps_to_migrate
                    .push(ClassMigration { diff: c.clone(), artifact_path: path.clone() });
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
            diff: contract.clone(),
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
