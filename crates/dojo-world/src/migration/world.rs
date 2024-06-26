use std::fmt::Display;
use std::mem;
use std::str::FromStr;

use anyhow::{bail, Result};
use starknet_crypto::FieldElement;
use topological_sort::TopologicalSort;

use super::class::ClassDiff;
use super::contract::ContractDiff;
use super::StateDiff;
use crate::manifest::utils::split_full_world_element_name;
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
                artifact_name: model.artifact_name.to_string(),
                name: model.inner.name.to_string(),
                namespace: model.inner.namespace.clone(),
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

        let contracts = local
            .contracts
            .iter()
            .map(|contract| {
                let base_class_hash = {
                    let class_hash = contract.inner.base_class_hash;
                    if class_hash != FieldElement::ZERO {
                        class_hash
                    } else {
                        *local.base.inner.class_hash()
                    }
                };

                ContractDiff {
                    artifact_name: contract.artifact_name.to_string(),
                    name: contract.inner.name.to_string(),
                    namespace: contract.inner.namespace.clone(),
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
                }
            })
            .collect::<Vec<_>>();

        let base = ClassDiff {
            artifact_name: local.base.artifact_name.clone(),
            name: BASE_CONTRACT_NAME.into(),
            namespace: "__DOJO__".to_string(),
            local_class_hash: *local.base.inner.class_hash(),
            original_class_hash: *local.base.inner.original_class_hash(),
            remote_class_hash: remote.as_ref().map(|m| *m.base.inner.class_hash()),
        };

        let world = ContractDiff {
            artifact_name: local.world.artifact_name.clone(),
            name: WORLD_CONTRACT_NAME.into(),
            namespace: "__DOJO__".to_string(),
            local_class_hash: *local.world.inner.class_hash(),
            original_class_hash: *local.world.inner.original_class_hash(),
            base_class_hash: *local.base.inner.class_hash(),
            remote_class_hash: remote.map(|m| *m.world.inner.class_hash()),
            init_calldata: vec![],
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

    pub fn update_order(&mut self, default_namespace: &str) -> Result<()> {
        let mut ts = TopologicalSort::<(String, String)>::new();

        // make the dependency graph by reading the constructor_calldata
        for contract in self.contracts.iter() {
            let cur_full_name = (contract.namespace.clone(), contract.name.clone());
            ts.insert(cur_full_name.clone());

            for field in &contract.init_calldata {
                if let Some(dependency) = field.strip_prefix("$contract_address:") {
                    let full_dep_name =
                        split_full_world_element_name(dependency, default_namespace)?;
                    ts.add_dependency(full_dep_name, cur_full_name.clone());
                } else if let Some(dependency) = field.strip_prefix("$class_hash:") {
                    let full_dep_name =
                        split_full_world_element_name(dependency, default_namespace)?;
                    ts.add_dependency(full_dep_name, cur_full_name.clone());
                } else {
                    // verify its a field element
                    match FieldElement::from_str(field) {
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

        for (c_namespace, c_name) in calculated_order {
            let contract = match self
                .contracts
                .iter()
                .find(|c| c.namespace == c_namespace && c.name == c_name)
            {
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

        for model in &self.models {
            writeln!(f, "{model}")?;
        }

        for contract in &self.contracts {
            writeln!(f, "{contract}")?;
        }

        Ok(())
    }
}
