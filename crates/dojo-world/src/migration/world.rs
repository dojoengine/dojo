use std::fmt::Display;

use super::class::ClassDiff;
use super::contract::ContractDiff;
use super::StateDiff;
use crate::manifest::{Manifest, BASE_CONTRACT_NAME, RESOURCE_METADATA_CONTRACT_NAME, WORLD_CONTRACT_NAME};

#[cfg(test)]
#[path = "world_test.rs"]
mod tests;

/// Represents the state differences between the local and remote worlds.
#[derive(Debug, Clone)]
pub struct WorldDiff {
    pub world: ContractDiff,
    pub base: ClassDiff,
    pub resource_metadata: ClassDiff,
    pub contracts: Vec<ContractDiff>,
    pub models: Vec<ClassDiff>,
}

impl WorldDiff {
    pub fn compute(local: Manifest, remote: Option<Manifest>) -> WorldDiff {
        let models = local
            .models
            .iter()
            .map(|model| ClassDiff {
                name: model.name.to_string(),
                local: model.class_hash,
                remote: remote.as_ref().and_then(|m| {
                    m.models.iter().find(|e| e.name == model.name).map(|s| s.class_hash)
                }),
            })
            .collect::<Vec<_>>();

        let contracts = local
            .contracts
            .iter()
            .map(|contract| ContractDiff {
                name: contract.name.to_string(),
                local: contract.class_hash,
                remote: remote.as_ref().and_then(|m| {
                    m.contracts
                        .iter()
                        .find(|r| r.class_hash == contract.class_hash)
                        .map(|r| r.class_hash)
                }),
            })
            .collect::<Vec<_>>();

        let base = ClassDiff {
            name: BASE_CONTRACT_NAME.into(),
            local: local.base.class_hash,
            remote: remote.as_ref().map(|m| m.base.class_hash),
        };

        let resource_metadata = ClassDiff {
            name: RESOURCE_METADATA_CONTRACT_NAME.into(),
            local: local.resource_metadata.class_hash,
            remote: remote.as_ref().map(|m| m.resource_metadata.class_hash),
        };

        let world = ContractDiff {
            name: WORLD_CONTRACT_NAME.into(),
            local: local.world.class_hash,
            remote: remote.map(|m| m.world.class_hash),
        };

        WorldDiff { world, base, resource_metadata, contracts, models }
    }

    pub fn count_diffs(&self) -> usize {
        let mut count = 0;

        if !self.world.is_same() {
            count += 1;
        }

        count += self.models.iter().filter(|s| !s.is_same()).count();
        count += self.contracts.iter().filter(|s| !s.is_same()).count();
        count
    }
}

impl Display for WorldDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.world)?;

        for model in &self.models {
            writeln!(f, "{model}")?;
        }

        for contract in &self.contracts {
            writeln!(f, "{contract}")?;
        }

        Ok(())
    }
}
