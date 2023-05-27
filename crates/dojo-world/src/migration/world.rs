use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::FieldElement;

use super::{ClassMigration, ContractMigration, Migration};
use crate::manifest::Manifest;
use crate::{EnvironmentConfig, WorldConfig};

#[cfg(test)]
#[path = "world_test.rs"]
mod test;

#[derive(Debug, Default, Clone)]
pub struct Contract {
    pub name: String,
    pub address: Option<FieldElement>,
    pub local: FieldElement,
    pub remote: Option<FieldElement>,
}

impl Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        if let Some(address) = self.address {
            writeln!(f, "   Address: {address:#x}",)?;
        }
        writeln!(f, "   Local: {:#x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: {remote:#x}")?;
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Class {
    pub name: String,
    pub local: FieldElement,
    pub remote: Option<FieldElement>,
}

impl Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        writeln!(f, "   Local: {:#x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: {remote:#x}")?;
        }

        Ok(())
    }
}

pub struct World {
    world: Contract,
    executor: Contract,
    contracts: Vec<Class>,
    components: Vec<Class>,
    systems: Vec<Class>,
    environment_config: EnvironmentConfig,
}

impl World {
    pub async fn from_path(
        target_dir: Utf8PathBuf,
        world_config: WorldConfig,
        env_config: EnvironmentConfig,
    ) -> Result<World> {
        let local_manifest = Manifest::load_from_path(target_dir.join("manifest.json"))?;

        let remote_manifest = if let Some(world_address) = world_config.address {
            let provider = env_config.provider()?;

            Manifest::from_remote(world_address, provider, Some(local_manifest.clone()))
                .await
                .map_err(|e| anyhow!("Problem creating remote manifest: {e}"))?
        } else {
            Manifest::default()
        };

        let systems = local_manifest
            .systems
            .iter()
            .map(|system| {
                Class {
                    // because the name returns by the `name` method of a
                    // system contract is without the 'System' suffix
                    name: system.name.strip_suffix("System").unwrap_or(&system.name).to_string(),
                    local: system.class_hash,
                    remote: remote_manifest
                        .systems
                        .iter()
                        .find(|e| e.name == system.name)
                        .map(|s| s.class_hash),
                }
            })
            .collect::<Vec<_>>();

        let components = local_manifest
            .components
            .iter()
            .map(|component| Class {
                name: component.name.to_string(),
                local: component.class_hash,
                remote: remote_manifest
                    .components
                    .iter()
                    .find(|e| e.name == component.name)
                    .map(|s| s.class_hash),
            })
            .collect::<Vec<_>>();

        let contracts = local_manifest
            .contracts
            .iter()
            .map(|contract| Class {
                name: contract.name.to_string(),
                local: contract.class_hash,
                remote: None,
            })
            .collect::<Vec<_>>();

        Ok(World {
            world: Contract {
                name: "World".into(),
                address: world_config.address,
                local: local_manifest.world.unwrap(),
                remote: remote_manifest.world,
            },
            executor: Contract {
                name: "Executor".into(),
                address: None,
                local: local_manifest.executor.unwrap(),
                remote: remote_manifest.executor,
            },
            systems,
            contracts,
            components,
            environment_config: env_config,
        })
    }

    /// evaluate which contracts/classes need to be (re)declared/deployed
    pub async fn prepare_for_migration(&self, target_dir: Utf8PathBuf) -> Result<Migration> {
        let entries = fs::read_dir(target_dir).unwrap_or_else(|error| {
            panic!("Problem reading source directory: {error}");
        });

        let mut artifact_paths = HashMap::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if file_name_str == "manifest.json" || !file_name_str.ends_with(".json") {
                continue;
            }

            let name =
                file_name_str.split('_').last().unwrap().trim_end_matches(".json").to_string();

            artifact_paths.insert(name, entry.path());
        }

        let world = evaluate_contract_for_migration(&self.world, &artifact_paths)?;
        let executor = evaluate_contract_for_migration(&self.executor, &artifact_paths)?;
        let components = evaluate_components_to_be_declared(&self.components, &artifact_paths)?;
        let systems = evaluate_systems_to_be_declared(&self.systems, &artifact_paths)?;

        let migrator = {
            let provider = self.environment_config.provider()?;
            let chain_id = provider.chain_id().await?;

            let account_address = self
                .environment_config
                .account_address
                .ok_or(anyhow!("missing account address for migration."))?;

            let signer = self.environment_config.signer()?;

            SingleOwnerAccount::new(provider, signer, account_address, chain_id)
        };

        Ok(Migration { world, executor, systems, components, migrator })
    }
}

impl Display for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.world)?;
        writeln!(f, "{}", self.executor)?;

        for component in &self.components {
            writeln!(f, "{component}")?;
        }

        for system in &self.systems {
            writeln!(f, "{system}")?;
        }

        for contract in &self.contracts {
            writeln!(f, "{contract}")?;
        }

        Ok(())
    }
}

fn evaluate_systems_to_be_declared(
    systems: &[Class],
    artifact_paths: &HashMap<String, PathBuf>,
) -> Result<Vec<ClassMigration>> {
    let mut syst_to_migrate: Vec<ClassMigration> = vec![];

    for s in systems {
        match s.remote {
            Some(remote) if remote == s.local => continue,
            _ => {
                let path = find_artifact_path(&format!("{}System", s.name), artifact_paths)?;
                syst_to_migrate.push(ClassMigration {
                    declared: false,
                    class: s.clone(),
                    artifact_path: path.clone(),
                });
            }
        }
    }

    Ok(syst_to_migrate)
}

fn evaluate_components_to_be_declared(
    components: &[Class],
    artifact_paths: &HashMap<String, PathBuf>,
) -> Result<Vec<ClassMigration>> {
    let mut comps_to_migrate: Vec<ClassMigration> = vec![];

    for c in components {
        match c.remote {
            Some(remote) if remote == c.local => continue,
            _ => {
                let path = find_artifact_path(&format!("{}Component", c.name), artifact_paths)?;
                comps_to_migrate.push(ClassMigration {
                    declared: false,
                    class: c.clone(),
                    artifact_path: path.clone(),
                });
            }
        }
    }

    Ok(comps_to_migrate)
}

// TODO: generate random salt if need to be redeployed
fn evaluate_contract_for_migration(
    contract: &Contract,
    artifact_paths: &HashMap<String, PathBuf>,
) -> Result<ContractMigration> {
    let should_deploy = if contract.address.is_none() {
        true
    } else {
        !matches!(contract.remote, Some(remote_hash) if remote_hash == contract.local)
    };

    let path = find_artifact_path(&contract.name, artifact_paths)?;

    Ok(ContractMigration {
        deployed: !should_deploy,
        contract: contract.clone(),
        artifact_path: path.clone(),
        contract_address: None,
        salt: FieldElement::ZERO,
    })
}

fn find_artifact_path<'a>(
    contract_name: &str,
    artifact_paths: &'a HashMap<String, PathBuf>,
) -> Result<&'a PathBuf> {
    artifact_paths
        .get(contract_name)
        .with_context(|| anyhow!("missing contract artifact for `{}` contract", contract_name))
}
