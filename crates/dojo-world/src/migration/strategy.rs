use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use starknet::core::types::FieldElement;
use starknet::core::utils::{cairo_short_string_to_felt, get_contract_address};
use starknet_crypto::{poseidon_hash_many, poseidon_hash_single};

use super::class::{ClassDiff, ClassMigration};
use super::contract::{ContractDiff, ContractMigration};
use super::world::WorldDiff;
use super::{DeployOutput, MigrationType, RegisterOutput};

#[derive(Debug)]
pub struct MigrationOutput {
    pub world: Option<DeployOutput>,
    pub executor: Option<DeployOutput>,
    pub contracts: Vec<DeployOutput>,
    pub models: Option<RegisterOutput>,
}

#[derive(Debug)]
pub struct MigrationStrategy {
    pub world_address: Option<FieldElement>,
    pub world: Option<ContractMigration>,
    pub executor: Option<ContractMigration>,
    pub base: Option<ClassMigration>,
    pub contracts: Vec<ContractMigration>,
    pub models: Vec<ClassMigration>,
}

#[derive(Debug)]
pub struct MigrationItemsInfo {
    pub new: usize,
    pub update: usize,
}

impl MigrationStrategy {
    pub fn world_address(&self) -> Result<FieldElement> {
        match &self.world {
            Some(c) => Ok(c.contract_address),
            None => self.world_address.ok_or(anyhow!("World address not found")),
        }
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

        self.contracts.iter().for_each(|item| match item.migration_type() {
            MigrationType::New => new += 1,
            MigrationType::Update => update += 1,
        });

        self.models.iter().for_each(|item| match item.migration_type() {
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
    seed: Option<FieldElement>,
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

        let name = file_name_str.trim_end_matches(".json").to_string();

        artifact_paths.insert(name, entry.path());
    }

    // We don't need to care if a contract has already been declared or not, because
    // the migration strategy will take care of that.

    // If the world contract needs to be migrated, then all contracts need to be migrated
    // else we need to evaluate which contracts need to be migrated.
    let mut world = evaluate_contract_to_migrate(&diff.world, &artifact_paths, false)?;
    let mut executor =
        evaluate_contract_to_migrate(&diff.executor, &artifact_paths, world.is_some())?;
    let base = evaluate_class_to_migrate(&diff.base, &artifact_paths, world.is_some())?;
    let contracts =
        evaluate_contracts_to_migrate(&diff.contracts, &artifact_paths, world.is_some())?;
    let models = evaluate_models_to_migrate(&diff.models, &artifact_paths, world.is_some())?;

    if let Some(executor) = &mut executor {
        executor.contract_address =
            get_contract_address(FieldElement::ZERO, diff.executor.local, &[], FieldElement::ZERO);
    }

    // If world needs to be migrated, then we expect the `seed` to be provided.
    if let Some(world) = &mut world {
        let salt =
            seed.map(poseidon_hash_single).ok_or(anyhow!("Missing seed for World deployment."))?;

        world.salt = salt;
        world.contract_address = get_contract_address(
            salt,
            diff.world.local,
            &[executor.as_ref().unwrap().contract_address, base.as_ref().unwrap().diff.local],
            FieldElement::ZERO,
        );
    }

    Ok(MigrationStrategy { world_address, world, executor, base, contracts, models })
}

fn evaluate_models_to_migrate(
    models: &[ClassDiff],
    artifact_paths: &HashMap<String, PathBuf>,
    world_contract_will_migrate: bool,
) -> Result<Vec<ClassMigration>> {
    let mut comps_to_migrate = vec![];

    for c in models {
        if let Ok(Some(c)) =
            evaluate_class_to_migrate(c, artifact_paths, world_contract_will_migrate)
        {
            comps_to_migrate.push(c);
        }
    }

    Ok(comps_to_migrate)
}

fn evaluate_class_to_migrate(
    class: &ClassDiff,
    artifact_paths: &HashMap<String, PathBuf>,
    world_contract_will_migrate: bool,
) -> Result<Option<ClassMigration>> {
    match class.remote {
        Some(remote) if remote == class.local && !world_contract_will_migrate => Ok(None),
        _ => {
            let path = find_artifact_path(class.name.as_str(), artifact_paths)?;
            Ok(Some(ClassMigration { diff: class.clone(), artifact_path: path.clone() }))
        }
    }
}

fn evaluate_contracts_to_migrate(
    contracts: &[ContractDiff],
    artifact_paths: &HashMap<String, PathBuf>,
    world_contract_will_migrate: bool,
) -> Result<Vec<ContractMigration>> {
    let mut comps_to_migrate = vec![];

    for c in contracts {
        match c.remote {
            Some(remote) if remote == c.local && !world_contract_will_migrate => continue,
            _ => {
                let path = find_artifact_path(c.name.as_str(), artifact_paths)?;
                comps_to_migrate.push(ContractMigration {
                    diff: c.clone(),
                    artifact_path: path.clone(),
                    salt: generate_salt(&c.name),
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
        || contract.remote.is_none()
        || matches!(contract.remote, Some(remote_hash) if remote_hash != contract.local)
    {
        let path = find_artifact_path(&contract.name, artifact_paths)?;

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

pub fn generate_salt(value: &str) -> FieldElement {
    poseidon_hash_many(
        &value
            .chars()
            .collect::<Vec<_>>()
            .chunks(31)
            .map(|chunk| {
                let s: String = chunk.iter().collect();
                cairo_short_string_to_felt(&s).unwrap()
            })
            .collect::<Vec<_>>(),
    )
}
