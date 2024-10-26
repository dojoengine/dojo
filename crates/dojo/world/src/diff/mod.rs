//! Represents the difference between a local and a remote world.
//!
//! The local and remote worlds are consumed to produce the diff, to avoid duplicating the
//! resources.

use std::collections::{HashMap, HashSet};

use compare::ComparableResource;
use dojo_types::naming;
use starknet::core::types::contract::SierraClass;
use starknet_crypto::Felt;

use super::local::{ResourceLocal, WorldLocal};
use super::remote::{ResourceRemote, WorldRemote};
use crate::utils::compute_dojo_contract_address;
use crate::{DojoSelector, ResourceType};

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
pub enum WorldStatus {
    /// The local world is a new version, and the remote world must be updated.
    /// (class_hash, casm_class_hash, sierra_class)
    NewVersion(Felt, Felt, SierraClass),
    /// The world is in sync with the remote world, same dojo version.
    Synced(Felt),
}

#[derive(Debug)]
pub struct WorldDiff {
    /// The status of the world.
    pub world_status: WorldStatus,
    /// The namespaces registered in the local world. A list of namespaces is kept
    /// additionally to the resources to ensure they can be processed first,
    /// since the all other resources are namespaced.
    pub namespaces: Vec<DojoSelector>,
    /// The resources registered in the local world, by dojo selector.
    pub resources: HashMap<DojoSelector, ResourceDiff>,
}

impl WorldDiff {
    /// Creates a new world diff from a local world.
    ///
    /// Consumes the local world to avoid duplicating the resources.
    pub fn from_local(local: WorldLocal) -> Self {
        let mut diff = Self {
            world_status: WorldStatus::NewVersion(
                local.class_hash.expect("World class hash must be set."),
                local.casm_class_hash.expect("World casm class hash must be set."),
                local.class.expect("World class must be set."),
            ),
            namespaces: vec![],
            resources: HashMap::new(),
        };

        for (selector, resource) in local.resources {
            if let ResourceLocal::Namespace(_) = &resource {
                diff.namespaces.push(selector);
            }

            diff.resources.insert(selector, ResourceDiff::Created(resource));
        }

        diff
    }

    /// Creates a new world diff from a local and a remote world.
    ///
    /// Consumes the local and remote worlds to avoid duplicating the resources,
    /// since the [`ResourceDiff`] will contain one or both of the local and remote resources.
    pub fn new(local: WorldLocal, mut remote: WorldRemote) -> Self {
        let local_world_class_hash = local.class_hash.expect("World class hash must be set.");
        let remote_world_class_hash =
            *remote.class_hashes.first().expect("Remote world must have at least one class hash.");

        let world_status = if local_world_class_hash == remote_world_class_hash {
            WorldStatus::Synced(local_world_class_hash)
        } else {
            WorldStatus::NewVersion(
                local_world_class_hash,
                local.casm_class_hash.expect("World casm class hash must be set."),
                local.class.expect("World class must be set."),
            )
        };

        let mut diff = Self { world_status, namespaces: vec![], resources: HashMap::new() };

        for (local_selector, local_resource) in local.resources {
            if let ResourceLocal::Namespace(_) = &local_resource {
                diff.namespaces.push(local_selector);
            }

            let remote_resource = remote.resources.remove(&local_selector);

            if let Some(remote_resource) = remote_resource {
                diff.resources.insert(local_selector, local_resource.compare(remote_resource));
            } else {
                diff.resources.insert(local_selector, ResourceDiff::Created(local_resource));
            }
        }

        diff
    }

    /// Returns whether the whole world is in sync.
    pub fn is_synced(&self) -> bool {
        matches!(self.world_status, WorldStatus::Synced(_))
            && self.resources.values().all(|resource| matches!(resource, ResourceDiff::Synced(_)))
    }

    /// Returns the remote writers of the resources.
    pub fn get_remote_writers(&self) -> HashMap<DojoSelector, HashSet<Felt>> {
        let mut remote_writers = HashMap::new();

        for resource in self.resources.values() {
            resource.update_remote_writers(&mut remote_writers);
        }

        remote_writers
    }

    /// Returns the remote owners of the resources.
    pub fn get_remote_owners(&self) -> HashMap<DojoSelector, HashSet<Felt>> {
        let mut remote_owners = HashMap::new();

        for resource in self.resources.values() {
            resource.update_remote_owners(&mut remote_owners);
        }

        remote_owners
    }

    /// Returns the deterministic addresses of the contracts based on the world address.
    pub fn get_contracts_addresses(&self, world_address: Felt) -> HashMap<DojoSelector, Felt> {
        let mut addresses = HashMap::new();

        for resource in self.resources.values() {
            if resource.resource_type() == ResourceType::Contract {
                let (selector, class_hash) = match resource {
                    ResourceDiff::Created(ResourceLocal::Contract(c)) => {
                        (c.dojo_selector(), c.class_hash)
                    }
                    ResourceDiff::Updated(_, ResourceRemote::Contract(c)) => {
                        (c.common.dojo_selector(), c.common.original_class_hash())
                    }
                    ResourceDiff::Synced(ResourceRemote::Contract(c)) => {
                        (c.common.dojo_selector(), c.common.original_class_hash())
                    }
                    _ => unreachable!(),
                };

                let address =
                    compute_dojo_contract_address(selector, class_hash.into(), world_address);

                addresses.insert(selector, address);
            }
        }

        addresses
    }

    /// Returns the resource diff from a name or tag.
    pub fn resource_diff_from_name_or_tag(&self, name_or_tag: &str) -> Option<&ResourceDiff> {
        let selector = if naming::is_valid_tag(name_or_tag) {
            naming::compute_selector_from_tag(name_or_tag)
        } else {
            naming::compute_bytearray_hash(name_or_tag)
        };

        self.resources.get(&selector)
    }
}

impl ResourceDiff {
    /// Updates the remote writers with the writers of the resource.
    pub fn update_remote_writers(&self, writers: &mut HashMap<DojoSelector, HashSet<Felt>>) {
        let (dojo_selector, remote_writers) = match self {
            ResourceDiff::Created(local) => (local.dojo_selector(), HashSet::new()),
            ResourceDiff::Updated(_, remote) => remote.get_writers(),
            ResourceDiff::Synced(remote) => remote.get_writers(),
        };

        writers
            .entry(dojo_selector)
            .and_modify(|remote: &mut HashSet<Felt>| remote.extend(remote_writers.clone()))
            .or_insert(remote_writers);
    }

    /// Updates the remote owners with the owners of the resource.
    pub fn update_remote_owners(&self, owners: &mut HashMap<DojoSelector, HashSet<Felt>>) {
        let (dojo_selector, remote_owners) = match self {
            ResourceDiff::Created(local) => (local.dojo_selector(), HashSet::new()),
            ResourceDiff::Updated(_, remote) => remote.get_owners(),
            ResourceDiff::Synced(remote) => remote.get_owners(),
        };

        owners
            .entry(dojo_selector)
            .and_modify(|remote: &mut HashSet<Felt>| remote.extend(remote_owners.clone()))
            .or_insert(remote_owners);
    }

    /// Returns the name of the resource.
    pub fn name(&self) -> String {
        match self {
            ResourceDiff::Created(local) => local.name(),
            ResourceDiff::Updated(local, _) => local.name(),
            ResourceDiff::Synced(remote) => remote.name(),
        }
    }

    /// Returns the namespace of the resource.
    pub fn namespace(&self) -> String {
        match self {
            ResourceDiff::Created(local) => local.namespace(),
            ResourceDiff::Updated(local, _) => local.namespace(),
            ResourceDiff::Synced(remote) => remote.namespace(),
        }
    }

    /// Returns the tag of the resource.
    pub fn tag(&self) -> String {
        match self {
            ResourceDiff::Created(local) => local.tag(),
            ResourceDiff::Updated(local, _) => local.tag(),
            ResourceDiff::Synced(remote) => remote.tag(),
        }
    }

    /// Returns the dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        match self {
            ResourceDiff::Created(local) => local.dojo_selector(),
            ResourceDiff::Updated(local, _) => local.dojo_selector(),
            ResourceDiff::Synced(remote) => remote.dojo_selector(),
        }
    }

    /// Returns the type of the resource.
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceDiff::Created(local) => local.resource_type(),
            ResourceDiff::Updated(_, remote) => remote.resource_type(),
            ResourceDiff::Synced(remote) => remote.resource_type(),
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use super::*;
    use crate::config::NamespaceConfig;
    use crate::local::{ContractLocal, NamespaceLocal, ResourceLocal, WorldLocal};
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
            namespace: ns.clone(),
            class: empty_sierra_class(),
            class_hash: Felt::ONE,
            casm_class_hash: Felt::ZERO,
        });

        local.add_resource(local_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.resources.len(), 1);
        assert!(matches!(
            diff.resources.get(&local_contract.dojo_selector()).unwrap(),
            ResourceDiff::Created(_)
        ));

        let remote_contract = ResourceRemote::Contract(ContractRemote {
            common: CommonResourceRemoteInfo::new(Felt::ONE, &ns, "c", Felt::ONE),
            is_initialized: false,
        });

        remote.add_resource(remote_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.resources.len(), 1);
        assert!(matches!(
            diff.resources.get(&local_contract.dojo_selector()).unwrap(),
            ResourceDiff::Synced(_)
        ));

        let mut local = WorldLocal::new(namespace_config);

        let local_contract = ResourceLocal::Contract(ContractLocal {
            name: "c".to_string(),
            namespace: ns.clone(),
            class: empty_sierra_class(),
            class_hash: Felt::TWO,
            casm_class_hash: Felt::ZERO,
        });

        local.add_resource(local_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.resources.len(), 1);
        assert!(matches!(
            diff.resources.get(&local_contract.dojo_selector()).unwrap(),
            ResourceDiff::Updated(_, _)
        ));
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
        assert!(matches!(
            diff.resources.get(&naming::compute_bytearray_hash("ns")).unwrap(),
            ResourceDiff::Created(_)
        ));
        assert!(matches!(
            diff.resources.get(&local_namespace.dojo_selector()).unwrap(),
            ResourceDiff::Created(_)
        ));

        let remote_namespace = ResourceRemote::Namespace(NamespaceRemote {
            name: "namespace1".to_string(),
            owners: HashSet::new(),
            writers: HashSet::new(),
        });

        remote.add_resource(remote_namespace.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.namespaces.len(), 2);
        assert!(matches!(
            diff.resources.get(&naming::compute_bytearray_hash("ns")).unwrap(),
            ResourceDiff::Created(_)
        ));
        assert!(matches!(
            diff.resources.get(&local_namespace.dojo_selector()).unwrap(),
            ResourceDiff::Synced(_)
        ));
    }
}
