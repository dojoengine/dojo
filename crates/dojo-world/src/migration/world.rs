use std::fmt::Display;
use std::mem;
use std::str::FromStr;

use anyhow::{bail, Result};
use starknet_crypto::Felt;
use topological_sort::TopologicalSort;

use super::class::ClassDiff;
use super::contract::ContractDiff;
use super::StateDiff;
use crate::contracts::naming;
use crate::manifest::{
    BaseManifest, DeploymentManifest, ManifestMethods, BASE_CONTRACT_TAG, WORLD_CONTRACT_TAG,
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
    pub events: Vec<ClassDiff>,
}

impl WorldDiff {
    pub fn compute(
        local: BaseManifest,
        remote: Option<DeploymentManifest>,
        default_namespace: &str,
    ) -> Result<WorldDiff> {
        let models = local
            .models
            .iter()
            .map(|model| ClassDiff {
                tag: model.inner.tag.to_string(),
                local_class_hash: *model.inner.class_hash(),
                original_class_hash: *model.inner.original_class_hash(),
                remote_class_hash: remote.as_ref().and_then(|m| {
                    m.models
                        .iter()
                        .find(|e| e.manifest_name == model.manifest_name)
                        .map(|s| *s.inner.class_hash())
                }),
            })
            .collect::<Vec<_>>();

        let events = local
            .events
            .iter()
            .map(|event| ClassDiff {
                tag: event.inner.tag.to_string(),
                local_class_hash: *event.inner.class_hash(),
                original_class_hash: *event.inner.original_class_hash(),
                remote_class_hash: remote.as_ref().and_then(|m| {
                    m.events
                        .iter()
                        .find(|e| e.manifest_name == event.manifest_name)
                        .map(|s| *s.inner.class_hash())
                }),
            })
            .collect::<Vec<_>>();

        let contracts = local
            .contracts
            .iter()
            .map(|contract| {
                let base_class_hash = {
                    let class_hash = contract.inner.base_class_hash;
                    if class_hash != Felt::ZERO {
                        class_hash
                    } else {
                        *local.base.inner.class_hash()
                    }
                };

                ContractDiff {
                    tag: contract.inner.tag.to_string(),
                    local_class_hash: *contract.inner.class_hash(),
                    original_class_hash: *contract.inner.original_class_hash(),
                    base_class_hash,
                    remote_class_hash: remote.as_ref().and_then(|m| {
                        m.contracts
                            .iter()
                            .find(|r| r.inner.class_hash() == contract.inner.class_hash())
                            .map(|r| *r.inner.class_hash())
                    }),
                    init_calldata: contract.inner.init_calldata.clone(),
                    local_writes: contract.inner.writes.clone(),
                    remote_writes: remote
                        .as_ref()
                        .and_then(|m| {
                            m.contracts
                                .iter()
                                .find(|r| r.inner.class_hash() == contract.inner.class_hash())
                                .map(|r| r.inner.writes.clone())
                        })
                        .unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>();

        let base = ClassDiff {
            tag: BASE_CONTRACT_TAG.to_string(),
            local_class_hash: *local.base.inner.class_hash(),
            original_class_hash: *local.base.inner.original_class_hash(),
            remote_class_hash: remote.as_ref().map(|m| *m.base.inner.class_hash()),
        };

        let world = ContractDiff {
            tag: WORLD_CONTRACT_TAG.to_string(),
            local_class_hash: *local.world.inner.class_hash(),
            original_class_hash: *local.world.inner.original_class_hash(),
            base_class_hash: *local.base.inner.class_hash(),
            remote_class_hash: remote.map(|m| *m.world.inner.class_hash()),
            init_calldata: vec![],
            local_writes: vec![],
            remote_writes: vec![],
        };

        let mut diff = WorldDiff { world, base, contracts, models, events };
        diff.update_order(default_namespace)?;

        Ok(diff)
    }

    pub fn count_diffs(&self) -> usize {
        let mut count = 0;

        if !self.world.is_same() {
            count += 1;
        }

        count += self.models.iter().filter(|s| !s.is_same()).count();
        count += self.events.iter().filter(|s| !s.is_same()).count();
        count += self.contracts.iter().filter(|s| !s.is_same()).count();
        count
    }

    pub fn update_order(&mut self, default_namespace: &str) -> Result<()> {
        let mut ts = TopologicalSort::<String>::new();

        // make the dependency graph by reading the constructor_calldata
        for contract in self.contracts.iter() {
            ts.insert(contract.tag.clone());

            for field in &contract.init_calldata {
                if let Some(dependency) = field.strip_prefix("$contract_address:") {
                    ts.add_dependency(
                        naming::ensure_namespace(dependency, default_namespace),
                        contract.tag.clone(),
                    );
                } else if let Some(dependency) = field.strip_prefix("$class_hash:") {
                    ts.add_dependency(
                        naming::ensure_namespace(dependency, default_namespace),
                        contract.tag.clone(),
                    );
                } else {
                    // verify its a field element
                    match Felt::from_str(field) {
                        Ok(_) => continue,
                        Err(e) => bail!(format!(
                            "Expected init_calldata element to be a special variable (i.e. \
                             starting with $contract_address or $class_hash) or be a \
                             FieldElement. Failed with error: {e:?}"
                        )),
                    }
                }
            }
        }

        let mut calculated_order = vec![];

        while !ts.is_empty() {
            let mut values = ts.pop_all();
            // if `ts` is not empty and `pop_all` returns an empty vector it means there is a cyclic
            // dependency see: https://docs.rs/topological-sort/latest/topological_sort/struct.TopologicalSort.html#method.pop_all
            if values.is_empty() {
                bail!("Cyclic dependency detected in `init_calldata`");
            }

            values.sort();
            calculated_order.extend(values);
        }

        let mut new_contracts = vec![];

        for tag in calculated_order {
            let contract = match self.contracts.iter().find(|c| c.tag == tag) {
                Some(c) => c,
                None => bail!("Unidentified contract found in `init_calldata`"),
            };

            new_contracts.push(contract.clone());
        }

        mem::swap(&mut self.contracts, &mut new_contracts);

        Ok(())
    }
}

impl Display for WorldDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.world)?;

        for event in &self.events {
            writeln!(f, "{event}")?;
        }

        for model in &self.models {
            writeln!(f, "{model}")?;
        }

        for contract in &self.contracts {
            writeln!(f, "{contract}")?;
        }

        Ok(())
    }
}
