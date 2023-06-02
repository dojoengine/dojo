use std::fmt::Display;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use starknet::core::types::FieldElement;

use crate::config::{EnvironmentConfig, WorldConfig};
use crate::manifest::Manifest;

/// Represents differences between a local and remote contract.
#[derive(Debug, Default, Clone)]
pub struct ContractDiff {
    pub name: String,
    pub local: FieldElement,
    pub remote: Option<FieldElement>,
    pub address: Option<FieldElement>,
}

/// Represents differences between a local and remote class.
#[derive(Debug, Default, Clone)]
pub struct ClassDiff {
    pub name: String,
    pub local: FieldElement,
    pub remote: Option<FieldElement>,
}

/// Represents the state differences between the local and remote worlds.
#[derive(Debug)]
pub struct WorldDiff {
    pub world: ContractDiff,
    pub executor: ContractDiff,
    pub contracts: Vec<ClassDiff>,
    pub components: Vec<ClassDiff>,
    pub systems: Vec<ClassDiff>,
}

impl WorldDiff {
    pub async fn from_path(
        target_dir: Utf8PathBuf,
        world_config: &WorldConfig,
        environment_config: &EnvironmentConfig,
    ) -> Result<WorldDiff> {
        let local_manifest = Manifest::load_from_path(target_dir.join("manifest.json"))?;

        let remote_manifest = if let Some(world_address) = world_config.address {
            let provider = environment_config.provider()?;
            Manifest::from_remote(provider, world_address, Some(local_manifest.clone()))
                .await
                .map(Some)
                .map_err(|e| anyhow!("Failed creating remote manifest: {e}"))?
        } else {
            None
        };

        let systems = local_manifest
            .systems
            .iter()
            .map(|system| {
                ClassDiff {
                    // because the name returns by the `name` method of a
                    // system contract is without the 'System' suffix
                    name: system.name.strip_suffix("System").unwrap_or(&system.name).to_string(),
                    local: system.class_hash,
                    remote: remote_manifest.as_ref().and_then(|m| {
                        m.systems.iter().find(|e| e.name == system.name).map(|s| s.class_hash)
                    }),
                }
            })
            .collect::<Vec<_>>();

        let components = local_manifest
            .components
            .iter()
            .map(|component| ClassDiff {
                name: component.name.to_string(),
                local: component.class_hash,
                remote: remote_manifest.as_ref().and_then(|m| {
                    m.components.iter().find(|e| e.name == component.name).map(|s| s.class_hash)
                }),
            })
            .collect::<Vec<_>>();

        let contracts = local_manifest
            .contracts
            .iter()
            .map(|contract| ClassDiff {
                name: contract.name.to_string(),
                local: contract.class_hash,
                remote: None,
            })
            .collect::<Vec<_>>();

        let world = ContractDiff {
            name: "World".into(),
            address: world_config.address,
            local: local_manifest.world,
            remote: remote_manifest.as_ref().map(|m| m.world),
        };

        let executor = ContractDiff {
            name: "Executor".into(),
            address: None,
            local: local_manifest.executor,
            remote: remote_manifest.map(|m| m.executor),
        };

        Ok(WorldDiff { world, executor, systems, contracts, components })
    }
}

impl Display for ContractDiff {
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

impl Display for ClassDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        writeln!(f, "   Local: {:#x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: {remote:#x}")?;
        }

        Ok(())
    }
}

impl Display for WorldDiff {
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
