use std::fmt::Display;

use convert_case::{Case, Casing};

use super::class::ClassDiff;
use super::contract::ContractDiff;
use super::StateDiff;
use crate::manifest::{
    BaseManifest, DeploymentManifest, ManifestMethods, BASE_CONTRACT_NAME, WORLD_CONTRACT_NAME,
};

#[cfg(test)]
#[path = "world_test.rs"]
mod tests;

/// Represents the state differences between the local and remote worlds.
#[derive(Debug, Clone)]
pub struct WorldDiff {
    pub world: ContractDiff,
    pub base: ClassDiff,
    pub contracts: Vec<ContractDiff>,
    pub models: Vec<ClassDiff>,
}

impl WorldDiff {
    pub fn compute(local: BaseManifest, remote: Option<DeploymentManifest>) -> WorldDiff {
        let models = local
            .models
            .iter()
            .map(|model| ClassDiff {
                name: model.name.to_string(),
                local: *model.inner.class_hash(),
                remote: remote.as_ref().and_then(|m| {
                    // Remote models are detected from events, where only the struct
                    // name (pascal case) is emitted.
                    // Local models uses the fully qualified name of the model,
                    // always in snake_case from cairo compiler.
                    let model_name = model
                        .name
                        .split("::")
                        .last()
                        .unwrap_or(&model.name)
                        .from_case(Case::Snake)
                        .to_case(Case::Pascal);

                    m.models.iter().find(|e| e.name == model_name).map(|s| *s.inner.class_hash())
                }),
            })
            .collect::<Vec<_>>();

        let contracts = local
            .contracts
            .iter()
            .map(|contract| ContractDiff {
                name: contract.name.to_string(),
                local: *contract.inner.class_hash(),
                remote: remote.as_ref().and_then(|m| {
                    m.contracts
                        .iter()
                        .find(|r| r.inner.class_hash() == contract.inner.class_hash())
                        .map(|r| *r.inner.class_hash())
                }),
            })
            .collect::<Vec<_>>();

        let base = ClassDiff {
            name: BASE_CONTRACT_NAME.into(),
            local: *local.base.inner.class_hash(),
            remote: remote.as_ref().map(|m| *m.base.inner.class_hash()),
        };

        let world = ContractDiff {
            name: WORLD_CONTRACT_NAME.into(),
            local: *local.world.inner.class_hash(),
            remote: remote.map(|m| *m.world.inner.class_hash()),
        };

        WorldDiff { world, base, contracts, models }
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
