//! Remote resources for the world, gathered from events emitted by the world at the given address.
//!
//! A remote resource must be reconstructible form the onchain world without any additional
//! information.
//!
//! Remote resources are coupled to the namespace used to register them. No resource can be
//! registered without a namespace (excepting namespaces themselves).

use std::collections::{HashMap, HashSet};

use starknet::core::types::Felt;

mod events_to_remote;
mod permissions;
mod resource;

pub use resource::*;

use crate::{ContractAddress, DojoSelector};

/// The remote world representation.
#[derive(Debug, Default, Clone)]
pub struct WorldRemote {
    /// The world's address used to build the remote world.
    pub address: Felt,
    /// The hash of the metadata associated to the world.
    pub metadata_hash: Felt,
    /// The class hashes of the world.
    pub class_hashes: Vec<Felt>,
    /// The resources of the world, by dojo selector.
    pub resources: HashMap<DojoSelector, ResourceRemote>,
    /// The deployed external contracts.
    pub deployed_external_contracts: Vec<String>,
    /// The declared external contract classes.
    pub declared_external_contract_classes: Vec<String>,
    /// Writers to resources that are not managed by the local project.
    pub external_writers: HashMap<DojoSelector, HashSet<ContractAddress>>,
    /// Owners of resources that are not managed by the local project.
    pub external_owners: HashMap<DojoSelector, HashSet<ContractAddress>>,
}

impl WorldRemote {
    /// Adds a resource to the world.
    pub fn add_resource(&mut self, resource: ResourceRemote) {
        self.resources.insert(resource.dojo_selector(), resource);
    }

    /// Returns the current class hash of the world.
    pub fn current_class_hash(&self) -> Felt {
        *self.class_hashes.last().expect("Remote world must have at least one class hash.")
    }
}

#[cfg(test)]
mod tests {
    use dojo_types::naming;

    use super::*;

    #[test]
    fn test_add_contract_resource() {
        let mut world_remote = WorldRemote::default();
        let namespace = "ns".to_string();

        let contract = ContractRemote {
            common: CommonRemoteInfo::new(Felt::ONE, &namespace, "c", Felt::ONE),
            is_initialized: false,
        };
        let resource = ResourceRemote::Contract(contract);

        world_remote.add_resource(resource);

        let selector = naming::compute_selector_from_names("ns", "c");
        assert!(world_remote.resources.contains_key(&selector));
    }

    #[test]
    fn test_add_model_resource() {
        let mut world_remote = WorldRemote::default();
        let namespace = "ns".to_string();

        let model =
            ModelRemote { common: CommonRemoteInfo::new(Felt::ONE, &namespace, "m", Felt::ONE) };
        let resource = ResourceRemote::Model(model);

        world_remote.add_resource(resource);

        let selector = naming::compute_selector_from_names("ns", "m");
        assert!(world_remote.resources.contains_key(&selector));
    }

    #[test]
    fn test_add_event_resource() {
        let mut world_remote = WorldRemote::default();
        let namespace = "ns".to_string();

        let event =
            EventRemote { common: CommonRemoteInfo::new(Felt::ONE, &namespace, "e", Felt::ONE) };
        let resource = ResourceRemote::Event(event);

        world_remote.add_resource(resource);

        let selector = naming::compute_selector_from_names("ns", "e");
        assert!(world_remote.resources.contains_key(&selector));
    }

    #[test]
    fn test_add_namespace_resource() {
        let mut world_remote = WorldRemote::default();
        let namespace = NamespaceRemote::new("ns".to_string());
        let resource = ResourceRemote::Namespace(namespace);

        world_remote.add_resource(resource);

        let selector = naming::compute_bytearray_hash("ns");
        assert!(world_remote.resources.contains_key(&selector));
    }
}
