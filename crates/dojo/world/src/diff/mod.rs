//! Represents the difference between a local and a remote world.
//!
//! The local and remote worlds are consumed to produce the diff, to avoid duplicating the
//! resources.

use std::collections::{HashMap, HashSet};

use compare::ComparableResource;
use starknet_crypto::Felt;

use super::local::{ResourceLocal, WorldLocal};
use super::remote::{ResourceRemote, WorldRemote};
use crate::{DojoSelector, Namespace};

mod compare;

/// The difference between a local and a remote resource.
///
/// The point of view is the local one.
/// Currently, having the remote resources that are not registered by the current project is not
/// supported, since a world can be permissionlessly updated by anyone.
#[derive(Debug)]
pub enum ResourceDiff {
    /// The resource has been created locally, and is not present in the remote world.
    Created(ResourceLocal),
    /// The resource has been updated locally, and is different from the remote world.
    Updated(ResourceLocal, ResourceRemote),
    /// The local resource is in sync with the remote world.
    Synced(ResourceRemote),
}

#[derive(Debug)]
pub struct WorldDiff {
    pub namespaces: Vec<ResourceDiff>,
    pub contracts: HashMap<Namespace, Vec<ResourceDiff>>,
    pub models: HashMap<Namespace, Vec<ResourceDiff>>,
    pub events: HashMap<Namespace, Vec<ResourceDiff>>,
}

impl WorldDiff {
    /// Creates a new world diff from a local and a remote world.
    ///
    /// Consumes the local and remote worlds to avoid duplicating the resources,
    /// since the [`ResourceDiff`] will contain one or both of the local and remote resources.
    pub fn new(mut local: WorldLocal, mut remote: WorldRemote) -> Self {
        let mut diff = Self {
            namespaces: vec![],
            contracts: HashMap::new(),
            models: HashMap::new(),
            events: HashMap::new(),
        };

        for local_ns in &local.namespaces {
            if remote.namespaces.contains(&local_ns) {
                diff.namespaces
                    .push(ResourceDiff::Synced(remote.resources.remove(&local_ns).unwrap()));
            } else {
                // The namespace is registered in the local world, safe to unwrap.
                diff.namespaces
                    .push(ResourceDiff::Created(local.resources.remove(&local_ns).unwrap()));
            }
        }

        compare_and_consume_resources(
            &local.contracts,
            &remote.contracts,
            &mut local.resources,
            &mut remote.resources,
            &mut diff.contracts,
        );

        compare_and_consume_resources(
            &local.models,
            &remote.models,
            &mut local.resources,
            &mut remote.resources,
            &mut diff.models,
        );

        compare_and_consume_resources(
            &local.events,
            &remote.events,
            &mut local.resources,
            &mut remote.resources,
            &mut diff.events,
        );

        diff
    }

    /// Returns the remote writers of the resources.
    pub fn get_remote_writers(&self) -> HashMap<DojoSelector, HashSet<Felt>> {
        let mut remote_writers = HashMap::new();

        for resource in &self.namespaces {
            resource.update_remote_writers("", &mut remote_writers);
        }

        for (namespace, contracts) in &self.contracts {
            for contract in contracts {
                contract.update_remote_writers(namespace, &mut remote_writers);
            }
        }

        for (namespace, models) in &self.models {
            for model in models {
                model.update_remote_writers(namespace, &mut remote_writers);
            }
        }

        for (namespace, events) in &self.events {
            for event in events {
                event.update_remote_writers(namespace, &mut remote_writers);
            }
        }

        remote_writers
    }

    /// Returns the remote owners of the resources.
    pub fn get_remote_owners(&self) -> HashMap<DojoSelector, HashSet<Felt>> {
        let mut remote_owners = HashMap::new();

        for resource in &self.namespaces {
            resource.update_remote_owners("", &mut remote_owners);
        }

        for (namespace, contracts) in &self.contracts {
            for contract in contracts {
                contract.update_remote_owners(namespace, &mut remote_owners);
            }
        }

        for (namespace, models) in &self.models {
            for model in models {
                model.update_remote_owners(namespace, &mut remote_owners);
            }
        }

        for (namespace, events) in &self.events {
            for event in events {
                event.update_remote_owners(namespace, &mut remote_owners);
            }
        }

        remote_owners
    }
}

/// Compares the local and remote resources and consumes them into a diff.
fn compare_and_consume_resources(
    local: &HashMap<Namespace, HashSet<DojoSelector>>,
    remote: &HashMap<Namespace, HashSet<DojoSelector>>,
    local_resources: &mut HashMap<DojoSelector, ResourceLocal>,
    remote_resources: &mut HashMap<DojoSelector, ResourceRemote>,
    diff: &mut HashMap<Namespace, Vec<ResourceDiff>>,
) {
    for (namespace, local_selectors) in local {
        for ls in local_selectors {
            let local_resource = local_resources.remove(ls).unwrap();

            let remote_selectors =
                if let Some(rss) = remote.get(namespace) { rss } else { &HashSet::new() };

            if remote_selectors.contains(ls) {
                diff.entry(namespace.clone()).or_default().push(
                    local_resource.compare(
                        remote_resources
                            .remove(ls)
                            .expect("Resource must exist if selector is present"),
                    ),
                );
            } else {
                diff.entry(namespace.clone())
                    .or_default()
                    .push(ResourceDiff::Created(local_resource));
            }
        }
    }
}

impl ResourceDiff {
    /// Updates the remote writers with the writers of the resource.
    pub fn update_remote_writers(
        &self,
        namespace: &str,
        writers: &mut HashMap<DojoSelector, HashSet<Felt>>,
    ) {
        let (dojo_selector, remote_writers) = match self {
            ResourceDiff::Created(local) => (local.dojo_selector(namespace), HashSet::new()),
            ResourceDiff::Updated(_, remote) => remote.get_writers(namespace),
            ResourceDiff::Synced(remote) => remote.get_writers(namespace),
        };

        writers
            .entry(dojo_selector)
            .and_modify(|remote: &mut HashSet<Felt>| remote.extend(remote_writers.clone()))
            .or_insert(remote_writers);
    }

    /// Updates the remote owners with the owners of the resource.
    pub fn update_remote_owners(
        &self,
        namespace: &str,
        owners: &mut HashMap<DojoSelector, HashSet<Felt>>,
    ) {
        let (dojo_selector, remote_owners) = match self {
            ResourceDiff::Created(local) => (local.dojo_selector(namespace), HashSet::new()),
            ResourceDiff::Updated(_, remote) => remote.get_owners(namespace),
            ResourceDiff::Synced(remote) => remote.get_owners(namespace),
        };

        owners
            .entry(dojo_selector)
            .and_modify(|remote: &mut HashSet<Felt>| remote.extend(remote_owners.clone()))
            .or_insert(remote_owners);
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use super::*;
    use crate::local::{ContractLocal, NamespaceConfig, NamespaceLocal, ResourceLocal, WorldLocal};
    use crate::remote::{CommonResourceRemoteInfo, ContractRemote, NamespaceRemote};
    use crate::test_utils::empty_sierra_class;

    #[test]
    fn test_world_diff_new() {
        let ns = "ns".to_string();
        let namespace_config = NamespaceConfig::new(&ns);
        let mut local = WorldLocal::new(namespace_config.clone());
        let mut remote = WorldRemote::default();

        let local_contract = ResourceLocal::Contract(ContractLocal {
            name: "c".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ONE,
            casm_class_hash: Felt::ZERO,
        });

        local.add_resource(local_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.contracts.len(), 1);
        assert_eq!(diff.contracts.get(&ns).unwrap().len(), 1);
        assert!(matches!(diff.contracts.get(&ns).unwrap()[0], ResourceDiff::Created(_)));

        let remote_contract = ResourceRemote::Contract(ContractRemote {
            common: CommonResourceRemoteInfo::new(Felt::ONE, "c".to_string(), Felt::ONE),
            initialized: false,
        });

        remote.add_resource(ns.clone(), remote_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.contracts.len(), 1);
        assert_eq!(diff.contracts.get(&ns).unwrap().len(), 1);
        assert!(matches!(diff.contracts.get(&ns).unwrap()[0], ResourceDiff::Synced(_)));

        let mut local = WorldLocal::new(namespace_config);

        let local_contract = ResourceLocal::Contract(ContractLocal {
            name: "c".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::TWO,
            casm_class_hash: Felt::ZERO,
        });

        local.add_resource(local_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.contracts.len(), 1);
        assert_eq!(diff.contracts.get(&ns).unwrap().len(), 1);
        assert!(matches!(diff.contracts.get(&ns).unwrap()[0], ResourceDiff::Updated(_, _)));
    }

    #[test]
    fn test_world_diff_namespace() {
        let ns = "ns".to_string();
        let namespace_config = NamespaceConfig::new(&ns);
        let mut local = WorldLocal::new(namespace_config.clone());
        let mut remote = WorldRemote::default();

        let local_namespace =
            ResourceLocal::Namespace(NamespaceLocal { name: "namespace1".to_string() });

        local.add_resource(local_namespace.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.namespaces.len(), 2);
        assert!(matches!(diff.namespaces[0], ResourceDiff::Created(_)));
        assert!(matches!(diff.namespaces[1], ResourceDiff::Created(_)));

        let remote_namespace = ResourceRemote::Namespace(NamespaceRemote {
            name: "namespace1".to_string(),
            owners: HashSet::new(),
            writers: HashSet::new(),
        });

        remote.add_resource(ns.clone(), remote_namespace.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.namespaces.len(), 2);
        assert!(matches!(diff.namespaces[0], ResourceDiff::Created(_)));
        assert!(matches!(diff.namespaces[1], ResourceDiff::Synced(_)));
    }
}
