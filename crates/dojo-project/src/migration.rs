use std::{env, fmt::Display};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use camino::Utf8PathBuf;
use dojo_lang::manifest::Manifest;
use scarb::{core::Config, ops, ui::Verbosity};
use starknet::{
    accounts::SingleOwnerAccount, core::types::FieldElement, providers::SequencerGatewayProvider,
    signers::LocalWallet,
};
use url::Url;

use crate::WorldConfig;

#[async_trait]
trait Declarable {
    async fn declare(&self, account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>);
}

#[async_trait]
trait Deployable: Declarable {
    async fn deploy(&self, account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>);
}

#[derive(Debug, Default)]
pub struct ContractMigration {
    deployed: bool,
    salt: FieldElement,
    contract: Contract,
}

#[derive(Debug, Default)]
pub struct ClassMigration {
    declared: bool,
    class: Class,
}

// TODO: migration config
#[derive(Debug, Default)]
pub struct WorldMigration {
    // rpc: Deployments,
    url: String, // sequencer url for testing purposes atm
    world: ContractMigration,
    executor: ContractMigration,
    store: ClassMigration,
    indexer: ClassMigration,
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,
}

// should only be created by calling `World::prepare_for_migration`
impl WorldMigration {
    pub async fn migrate(&self) {
        if self.world.deployed {}
    }
}

#[derive(Debug, Default, Clone)]
struct Contract {
    name: String,
    address: Option<FieldElement>,
    local: FieldElement,
    remote: Option<FieldElement>,
}

impl Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        if let Some(address) = self.address {
            writeln!(f, "   Address: 0x{:x}", address)?;
        }
        writeln!(f, "   Local: 0x{:x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: 0x{:x}", remote)?;
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
struct Class {
    world: FieldElement,
    name: String,
    local: FieldElement,
    remote: Option<FieldElement>,
}

impl Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        writeln!(f, "   Local: 0x{:x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: 0x{:x}", remote)?;
        }

        Ok(())
    }
}

pub struct World {
    world: Contract,
    executor: Contract,
    indexer: Class,
    store: Class,
    contracts: Vec<Class>,
    components: Vec<Class>,
    systems: Vec<Class>,
}

impl World {
    pub async fn from_path(source_dir: Utf8PathBuf) -> Result<World> {
        let url = Url::parse("https://starknet-goerli.cartridge.gg/").unwrap();

        let manifest_path = source_dir.join("Scarb.toml");
        let config = Config::builder(manifest_path)
            .ui_verbosity(Verbosity::Verbose)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .build()
            .unwrap();
        let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });
        let world_config = WorldConfig::from_workspace(&ws).unwrap_or_default();

        let local_manifest =
            Manifest::load_from_path(source_dir.join("target/release/manifest.json"))?;

        let remote_manifest = if let Some(world_address) = world_config.address {
            Manifest::from_remote(world_address, url, &local_manifest)
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
                    world: world_config.address.unwrap(),
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
                world: world_config.address.unwrap(),
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
                world: world_config.address.unwrap(),
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
                local: local_manifest.world.unwrap(),
                remote: remote_manifest.world,
            },
            indexer: Class {
                world: world_config.address.unwrap(),
                name: "Indexer".into(),
                local: local_manifest.indexer.unwrap(),
                remote: remote_manifest.indexer,
            },
            store: Class {
                world: world_config.address.unwrap(),
                name: "Store".into(),
                local: local_manifest.store.unwrap(),
                remote: remote_manifest.store,
            },
            systems,
            contracts,
            components,
        })
    }

    /// evaluate which contracts/classes need to be (re)declared/deployed
    pub fn prepare_for_migration(&self) -> WorldMigration {
        let world_migration = if self.world.address.is_none() {
            ContractMigration { contract: self.world.clone(), ..Default::default() }
        } else {
            match self.world.remote {
                Some(remote_hash) if remote_hash == self.world.local => ContractMigration {
                    deployed: true,
                    contract: self.world.clone(),
                    ..Default::default()
                },
                _ => ContractMigration { contract: self.world.clone(), ..Default::default() },
            }
        };

        let executor_migration = if self.executor.address.is_none() {
            ContractMigration { contract: self.executor.clone(), ..Default::default() }
        } else {
            match self.executor.remote {
                Some(remote_hash) if remote_hash == self.executor.local => ContractMigration {
                    deployed: true,
                    contract: self.world.clone(),
                    ..Default::default()
                },
                _ => ContractMigration { contract: self.executor.clone(), ..Default::default() },
            }
        };

        let store_migration = if self.store.remote.is_none() {
            ClassMigration { class: self.store.clone(), declared: false }
        } else {
            match self.store.remote {
                Some(store_class_hash) if store_class_hash == self.store.local => {
                    ClassMigration { class: self.store.clone(), declared: true }
                }
                _ => ClassMigration { class: self.store.clone(), declared: false },
            }
        };

        let indexer_migration = if self.indexer.remote.is_none() {
            ClassMigration { class: self.store.clone(), declared: false }
        } else {
            match self.indexer.remote {
                Some(indexer_class_hash) if indexer_class_hash == self.indexer.local => {
                    ClassMigration { class: self.indexer.clone(), declared: true }
                }
                _ => ClassMigration { class: self.indexer.clone(), declared: false },
            }
        };

        WorldMigration {
            world: world_migration,
            store: store_migration,
            indexer: indexer_migration,
            executor: executor_migration,
            systems: self.find_systems_to_be_declared(),
            components: self.find_components_to_be_declared(),
            ..Default::default()
        }
    }

    fn find_components_to_be_declared(&self) -> Vec<ClassMigration> {
        self.components
            .iter()
            .filter_map(|c| {
                c.remote.and_then(|remote_hash| {
                    if remote_hash == c.local {
                        None
                    } else {
                        Some(ClassMigration { declared: false, class: c.clone() })
                    }
                })
            })
            .collect()
    }

    fn find_systems_to_be_declared(&self) -> Vec<ClassMigration> {
        self.systems
            .iter()
            .filter_map(|c| {
                c.remote.and_then(|remote_hash| {
                    if remote_hash == c.local {
                        None
                    } else {
                        Some(ClassMigration { declared: false, class: c.clone() })
                    }
                })
            })
            .collect()
    }
}

impl Display for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.world)?;
        writeln!(f, "{}", self.executor)?;
        writeln!(f, "{}", self.store)?;
        writeln!(f, "{}", self.indexer)?;

        for component in &self.components {
            writeln!(f, "{}", component)?;
        }

        for system in &self.systems {
            writeln!(f, "{}", system)?;
        }

        for contract in &self.contracts {
            writeln!(f, "{}", contract)?;
        }

        Ok(())
    }
}
