use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8PathBuf;
use starknet::core::types::FieldElement;
use starknet::core::utils::{cairo_short_string_to_felt, get_contract_address};
use starknet_crypto::{poseidon_hash_many, poseidon_hash_single};

use super::class::{ClassDiff, ClassMigration};
use super::contract::{ContractDiff, ContractMigration};
use super::world::WorldDiff;
use super::MigrationType;

#[derive(Debug, Clone)]
pub enum MigrationMetadata {
    Contract(ContractDiff),
}

#[derive(Debug, Clone)]
pub struct MigrationStrategy {
    pub world_address: Option<FieldElement>,
    pub world: Option<ContractMigration>,
    pub base: Option<ClassMigration>,
    pub contracts: Vec<ContractMigration>,
    pub models: Vec<ClassMigration>,
    pub metadata: HashMap<String, MigrationMetadata>,
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

    pub fn resolve_variable(&mut self, world_address: FieldElement) -> Result<()> {
        for contract in self.contracts.iter_mut() {
            for field in contract.diff.init_calldata.iter_mut() {
                if let Some(dependency) = field.strip_prefix("$contract_address:") {
                    let dependency_contract = self.metadata.get(dependency).unwrap();

                    match dependency_contract {
                        MigrationMetadata::Contract(c) => {
                            let contract_address = get_contract_address(
                                generate_salt(&c.name),
                                c.base_class_hash,
                                &[],
                                world_address,
                            );
                            *field = contract_address.to_string();
                        }
                    }
                } else if let Some(dependency) = field.strip_prefix("$class_hash:") {
                    let dependency_contract = self.metadata.get(dependency).unwrap();
                    match dependency_contract {
                        MigrationMetadata::Contract(c) => {
                            *field = c.local_class_hash.to_string();
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// construct migration strategy
/// evaluate which contracts/classes need to be declared/deployed
pub fn prepare_for_migration(
    world_address: Option<FieldElement>,
    seed: FieldElement,
    target_dir: &Utf8PathBuf,
    diff: WorldDiff,
) -> Result<MigrationStrategy> {
    let mut metadata = HashMap::new();
    let entries = fs::read_dir(target_dir).with_context(|| {
        format!(
            "Failed trying to read target directory ({target_dir})\nNOTE: build files are profile \
             specified so make sure to run build command with correct profile. For e.g. `sozo -P \
             my_profile build`"
        )
    })?;

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
    let base = evaluate_class_to_migrate(&diff.base, &artifact_paths, world.is_some())?;
    let contracts = evaluate_contracts_to_migrate(
        &diff.contracts,
        &artifact_paths,
        &mut metadata,
        world.is_some(),
    )?;
    let models = evaluate_models_to_migrate(&diff.models, &artifact_paths, world.is_some())?;

    // If world needs to be migrated, then we expect the `seed` to be provided.
    if let Some(world) = &mut world {
        let salt = poseidon_hash_single(seed);

        world.salt = salt;
        let generated_world_address = get_contract_address(
            salt,
            diff.world.original_class_hash,
            &[base.as_ref().unwrap().diff.original_class_hash],
            FieldElement::ZERO,
        );

        if let Some(world_address) = world_address {
            if world_address != generated_world_address {
                println!("generated_world_address: {:?}", generated_world_address);
                bail!(
                    "Calculated world address doesn't match provided world address.\nIf you are \
                     deploying with custom seed make sure `world_address` is correctly configured \
                     (or not set) `Scarb.toml`"
                )
            }
        }
        world.contract_address = generated_world_address;
    }

    Ok(MigrationStrategy { world_address, world, base, contracts, models, metadata })
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
    match class.remote_class_hash {
        Some(remote) if remote == class.local_class_hash && !world_contract_will_migrate => {
            Ok(None)
        }
        _ => {
            let path = find_artifact_path(class.name.as_str(), artifact_paths)?;
            Ok(Some(ClassMigration { diff: class.clone(), artifact_path: path.clone() }))
        }
    }
}

fn evaluate_contracts_to_migrate(
    contracts: &[ContractDiff],
    artifact_paths: &HashMap<String, PathBuf>,
    metadata: &mut HashMap<String, MigrationMetadata>,
    world_contract_will_migrate: bool,
) -> Result<Vec<ContractMigration>> {
    let mut comps_to_migrate = vec![];

    for c in contracts {
        metadata.insert(c.name.clone(), MigrationMetadata::Contract(c.clone()));
        match c.remote_class_hash {
            Some(remote) if remote == c.local_class_hash && !world_contract_will_migrate => {
                continue;
            }
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
        || contract.remote_class_hash.is_none()
        || matches!(contract.remote_class_hash, Some(remote_hash) if remote_hash != contract.local_class_hash)
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
