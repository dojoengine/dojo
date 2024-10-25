//! Local resources for the world, gathered from the Scarb generated artifacts.
//!
//! When resources are compiled, there's no namespace attached to them.
//! However, to be registered and used in the world, they need to be namespaced.
//! To link a local resource to its world representation, a namespace configuration
//! is needed.
//!
//! Class hashes are cached into the resource to avoid recomputing them when
//! requesting it.

use std::collections::{HashMap, HashSet};

use dojo_types::naming;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::Felt;
use starknet::core::utils::{self as snutils, CairoShortStringToFeltError};
use starknet_crypto::poseidon_hash_single;

mod artifact_to_local;

use crate::config::NamespaceConfig;
use crate::utils::compute_world_address;
use crate::{DojoSelector, Namespace, ResourceType};

/// A local resource.
#[derive(Debug, Clone)]
pub enum ResourceLocal {
    Namespace(NamespaceLocal),
    Contract(ContractLocal),
    Model(ModelLocal),
    Event(EventLocal),
    Starknet(StarknetLocal),
}

#[derive(Debug, Clone, Default)]
pub struct WorldLocal {
    /// The class hash of the world.
    /// We use an option here since [`SierraClass`] doesn't implement default
    /// and it's easier to handle the option than having a default value to know
    /// if the world class has been set or not.
    pub class: Option<SierraClass>,
    /// The class hash of the world.
    pub class_hash: Option<Felt>,
    /// The casm class hash of the world.
    pub casm_class_hash: Option<Felt>,
    /// The resources of the world.
    pub resources: HashMap<DojoSelector, ResourceLocal>,
    /// The namespace configuration.
    pub namespace_config: NamespaceConfig,
}

#[derive(Debug, Clone)]
pub struct ContractLocal {
    /// The name of the contract.
    pub name: String,
    /// The namespace on which the contract is willing to be registered.
    pub namespace: String,
    /// The class of the contract.
    pub class: SierraClass,
    /// The class hash of the contract.
    pub class_hash: Felt,
    /// The casm class hash of the contract.
    pub casm_class_hash: Felt,
    // TODO: add systems for better debugging/more info for users.
}

#[derive(Debug, Clone)]
pub struct ModelLocal {
    /// The name of the model.
    pub name: String,
    /// The namespace on which the model is willing to be registered.
    pub namespace: String,
    /// The class of the model.
    pub class: SierraClass,
    /// The class hash of the model.
    pub class_hash: Felt,
    /// The casm class hash of the model.
    pub casm_class_hash: Felt,
}

#[derive(Debug, Clone)]
pub struct EventLocal {
    /// The name of the event.
    pub name: String,
    /// The namespace on which the event is willing to be registered.
    pub namespace: String,
    /// The class of the event.
    pub class: SierraClass,
    /// The class hash of the event.
    pub class_hash: Felt,
    /// The casm class hash of the event.
    pub casm_class_hash: Felt,
}

#[derive(Debug, Clone)]
pub struct StarknetLocal {
    /// The name of the starknet contract.
    pub name: String,
    /// The namespace on which the starknet contract is willing to be registered.
    pub namespace: String,
    /// The class of the starknet contract.
    pub class: SierraClass,
    /// The class hash of the starknet contract.
    pub class_hash: Felt,
    /// The casm class hash of the starknet contract.
    pub casm_class_hash: Felt,
}

#[derive(Debug, Clone)]
pub struct NamespaceLocal {
    /// The name of the namespace.
    pub name: String,
}

impl ResourceLocal {
    /// Returns the name of the resource.
    pub fn name(&self) -> String {
        match self {
            ResourceLocal::Contract(c) => c.name.clone(),
            ResourceLocal::Model(m) => m.name.clone(),
            ResourceLocal::Event(e) => e.name.clone(),
            ResourceLocal::Starknet(s) => s.name.clone(),
            ResourceLocal::Namespace(n) => n.name.clone(),
        }
    }

    /// Returns the namespace of the resource.
    pub fn namespace(&self) -> String {
        match self {
            ResourceLocal::Namespace(n) => n.name.clone(),
            ResourceLocal::Contract(c) => c.namespace.clone(),
            ResourceLocal::Model(m) => m.namespace.clone(),
            ResourceLocal::Event(e) => e.namespace.clone(),
            ResourceLocal::Starknet(s) => s.namespace.clone(),
        }
    }

    /// Returns the class hash of the resource.
    pub fn class_hash(&self) -> Felt {
        match self {
            ResourceLocal::Contract(c) => c.class_hash,
            ResourceLocal::Model(m) => m.class_hash,
            ResourceLocal::Event(e) => e.class_hash,
            ResourceLocal::Starknet(s) => s.class_hash,
            _ => Felt::ZERO,
        }
    }

    /// Returns the dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        match self {
            ResourceLocal::Namespace(n) => naming::compute_bytearray_hash(&n.name),
            _ => naming::compute_selector_from_names(&self.namespace(), &self.name()),
        }
    }

    /// Returns the tag of the resource.
    pub fn tag(&self) -> String {
        naming::get_tag(&self.namespace(), &self.name())
    }

    /// Returns the contract resource.
    ///
    /// This function panics since it must only be used where the developer
    /// can ensure that the resource is a contract.
    pub fn as_contract(&self) -> &ContractLocal {
        match self {
            ResourceLocal::Contract(c) => c,
            _ => panic!("Resource {} is not a contract", self.name()),
        }
    }

    /// Returns the type of the resource.
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceLocal::Contract(_) => ResourceType::Contract,
            ResourceLocal::Model(_) => ResourceType::Model,
            ResourceLocal::Event(_) => ResourceType::Event,
            ResourceLocal::Namespace(_) => ResourceType::Namespace,
            ResourceLocal::Starknet(_) => ResourceType::StarknetContract,
        }
    }
}

impl ContractLocal {
    /// Returns the dojo selector of the contract.
    pub fn dojo_selector(&self) -> DojoSelector {
        naming::compute_selector_from_names(&self.namespace, &self.name)
    }
}

impl WorldLocal {
    /// Creates a new world local with a namespace configuration.
    pub fn new(namespace_config: NamespaceConfig) -> Self {
        let mut world = Self {
            namespace_config: namespace_config.clone(),
            class: None,
            class_hash: None,
            casm_class_hash: None,
            resources: HashMap::new(),
        };

        for namespace in namespace_config.list_namespaces() {
            world
                .add_resource(ResourceLocal::Namespace(NamespaceLocal { name: namespace.clone() }));
        }

        world
    }

    /// Computes the deterministic address of the world contract based on the given seed.
    ///
    /// If a project has a local world contract that is a different class hash from the one
    /// used for the initial deployment, the address will be different. The user must explicitly
    /// provide the world address in that case.
    pub fn compute_world_address(&self, seed: &str) -> Result<Felt, CairoShortStringToFeltError> {
        let class_hash = self.class_hash.expect("World must have a class hash.");
        compute_world_address(seed, class_hash)
    }

    /// Adds a resource to the world local.
    pub fn add_resource(&mut self, resource: ResourceLocal) {
        if let ResourceLocal::Namespace(namespace) = &resource {
            let selector = naming::compute_bytearray_hash(&namespace.name);
            self.resources.insert(selector, resource);
            return;
        }

        self.resources.insert(resource.dojo_selector(), resource);
    }

    /// Returns the contract resource.
    ///
    /// This function panics since it must only be used where the developer
    /// can ensure that the resource is a contract.
    pub fn get_contract_resource(&self, selector: DojoSelector) -> &ContractLocal {
        self.resources
            .get(&selector)
            .expect(&format!("Contract with selector {:#x} not found", selector))
            .as_contract()
    }

    /// Returns the resource from a name or tag.
    pub fn resource_from_name_or_tag(&self, name_or_tag: &str) -> Option<&ResourceLocal> {
        let selector = if naming::is_valid_tag(name_or_tag) {
            naming::compute_selector_from_tag(name_or_tag)
        } else {
            naming::compute_selector_from_tag(name_or_tag)
        };

        self.resources.get(&selector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::empty_sierra_class;

    #[test]
    fn test_add_resource() {
        let mut world = WorldLocal::new(NamespaceConfig::new("dojo"));

        assert_eq!(world.namespaces.len(), 1);
        assert_eq!(world.resources.len(), 1);

        let n = world.resources.get(&naming::compute_bytearray_hash("dojo")).unwrap();
        assert_eq!(n.name(), "dojo");

        world.add_resource(ResourceLocal::Contract(ContractLocal {
            name: "c1".to_string(),
            namespace: "dojo".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ZERO,
            casm_class_hash: Felt::ZERO,
        }));

        let selector = naming::compute_selector_from_names(&"dojo".to_string(), &"c1".to_string());

        assert_eq!(world.contracts.get("dojo").unwrap().len(), 1);
        assert_eq!(world.contracts.get("dojo").unwrap().contains(&selector), true);
        assert_eq!(world.resources.len(), 1);

        world.add_resource(ResourceLocal::Contract(ContractLocal {
            name: "c2".to_string(),
            namespace: "dojo".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ZERO,
            casm_class_hash: Felt::ZERO,
        }));

        let selector2 = naming::compute_selector_from_names(&"dojo".to_string(), &"c2".to_string());

        assert_eq!(world.contracts.get("dojo").unwrap().len(), 2);
        assert_eq!(world.resources.len(), 2);
        assert_eq!(world.contracts.get("dojo").unwrap().contains(&selector), true);
        assert_eq!(world.contracts.get("dojo").unwrap().contains(&selector2), true);
    }
}
