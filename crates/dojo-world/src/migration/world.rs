use std::fmt::Display;

use super::class::ClassDiff;
use super::contract::ContractDiff;
use super::StateDiff;
use crate::manifest::{Manifest, EXECUTOR_CONTRACT_NAME, WORLD_CONTRACT_NAME};

#[cfg(test)]
#[path = "world_test.rs"]
mod tests;

/// Represents the state differences between the local and remote worlds.
#[derive(Debug, Clone)]
pub struct WorldDiff {
    pub world: ContractDiff,
    pub executor: ContractDiff,
    pub contracts: Vec<ContractDiff>,
    pub components: Vec<ClassDiff>,
    pub systems: Vec<ClassDiff>,
}

impl WorldDiff {
    pub fn compute(local: Manifest, remote: Option<Manifest>) -> WorldDiff {
        let systems = local
            .systems
            .iter()
            .map(|system| {
                ClassDiff {
                    // because the name returns by the `name` method of a
                    // system contract is without the 'System' suffix
                    name: system.name.strip_suffix("System").unwrap_or(&system.name).to_string(),
                    local: system.class_hash,
                    remote: remote.as_ref().and_then(|m| {
                        m.systems.iter().find(|e| e.name == system.name).map(|s| s.class_hash)
                    }),
                }
            })
            .collect::<Vec<_>>();

        let components = local
            .components
            .iter()
            .map(|component| ClassDiff {
                name: component.name.to_string(),
                local: component.class_hash,
                remote: remote.as_ref().and_then(|m| {
                    m.components.iter().find(|e| e.name == component.name).map(|s| s.class_hash)
                }),
            })
            .collect::<Vec<_>>();

        let contracts = local
            .contracts
            .iter()
            .map(|contract| ContractDiff {
                name: contract.name.to_string(),
                local: contract.class_hash,
                remote: None,
            })
            .collect::<Vec<_>>();

        let executor = ContractDiff {
            name: EXECUTOR_CONTRACT_NAME.into(),
            local: local.executor.class_hash,
            remote: remote.as_ref().map(|m| m.executor.class_hash),
        };

        let world = ContractDiff {
            name: WORLD_CONTRACT_NAME.into(),
            local: local.world.class_hash,
            remote: remote.map(|m| m.world.class_hash),
        };

        WorldDiff { world, executor, systems, contracts, components }
    }

    pub fn count_diffs(&self) -> usize {
        let mut count = 0;

        if !self.world.is_same() {
            count += 1;
        }

        if !self.executor.is_same() {
            count += 1;
        }

        count += self.systems.iter().filter(|s| !s.is_same()).count();
        count += self.components.iter().filter(|s| !s.is_same()).count();
        count += self.contracts.iter().filter(|s| !s.is_same()).count();
        count
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
