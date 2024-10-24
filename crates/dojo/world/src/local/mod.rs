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

mod artifact_to_local;
mod namespace_config;

pub use namespace_config::NamespaceConfig;

use crate::{DojoSelector, Namespace};

/// A local resource.
#[derive(Debug, Clone)]
pub enum ResourceLocal {
    Namespace(NamespaceLocal),
    Contract(ContractLocal),
    Model(ModelLocal),
    Event(EventLocal),
    Starknet(StarknetLocal),
}

#[derive(Debug)]
pub struct WorldLocal {
    /// The class hash of the world.
    /// We use an option here since [`SierraClass`] doesn't implement default
    /// and it's easier to handle the option than having a default value to know
    /// if the world class has been set or not.
    pub class: Option<SierraClass>,
    /// The namespaces of the world.
    pub namespaces: HashSet<DojoSelector>,
    /// The contracts of the world.
    pub contracts: HashMap<Namespace, HashSet<DojoSelector>>,
    /// The models of the world.
    pub models: HashMap<Namespace, HashSet<DojoSelector>>,
    /// The events of the world.
    pub events: HashMap<Namespace, HashSet<DojoSelector>>,
    /// The starknet contracts of the world.
    pub starknet_contracts: HashMap<Namespace, HashSet<DojoSelector>>,
    /// The resources of the world.
    pub resources: HashMap<DojoSelector, ResourceLocal>,
    /// The namespace configuration.
    pub namespace_config: NamespaceConfig,
}

#[derive(Debug, Clone)]
pub struct ContractLocal {
    /// The name of the contract.
    pub name: String,
    /// The class of the contract.
    pub class: SierraClass,
    /// The class hash of the contract.
    pub class_hash: Felt,
    // TODO: add systems for better debugging/more info for users.
}

#[derive(Debug, Clone)]
pub struct ModelLocal {
    /// The name of the model.
    pub name: String,
    /// The class of the model.
    pub class: SierraClass,
    /// The class hash of the model.
    pub class_hash: Felt,
}

#[derive(Debug, Clone)]
pub struct EventLocal {
    /// The name of the event.
    pub name: String,
    /// The class of the event.
    pub class: SierraClass,
    /// The class hash of the event.
    pub class_hash: Felt,
}

#[derive(Debug, Clone)]
pub struct StarknetLocal {
    /// The name of the starknet contract.
    pub name: String,
    /// The class of the starknet contract.
    pub class: SierraClass,
    /// The class hash of the starknet contract.
    pub class_hash: Felt,
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
}

impl WorldLocal {
    /// Creates a new world local with a namespace configuration.
    pub fn new(namespace_config: NamespaceConfig) -> Self {
        let mut world = Self {
            namespace_config: namespace_config.clone(),
            class: None,
            namespaces: HashSet::new(),
            contracts: HashMap::new(),
            models: HashMap::new(),
            events: HashMap::new(),
            starknet_contracts: HashMap::new(),
            resources: HashMap::new(),
        };

        for namespace in namespace_config.list_namespaces() {
            world.add_resource(ResourceLocal::Namespace(NamespaceLocal {
                name: namespace.clone(),
            }));
        }

        world
    }

    /// Adds a resource to the world local.
    pub fn add_resource(&mut self, resource: ResourceLocal) {
        let name = resource.name();
        let namespaces = self.namespace_config.get_namespaces(&name);

        if let ResourceLocal::Namespace(namespace) = &resource {
            let selector = naming::compute_bytearray_hash(&namespace.name);
            self.namespaces.insert(selector);
            self.resources.insert(selector, resource.clone());
            return;
        }

        for namespace in namespaces {
            let selector = naming::compute_selector_from_names(&namespace, &name);
            // Not the most efficient, but it's not the most critical path.
            // We could have done a mapping of <Name, Resource> but this adds an additional lookup
            // with the current datastructure, since the [`DojoSelector`] doesn't contain the name in clear,
            // we have to lookup all the resources to find out the name matching.
            self.resources.insert(selector, resource.clone());

            match resource {
                ResourceLocal::Contract(_) => {
                    self.contracts.entry(namespace).or_insert_with(HashSet::new).insert(selector);
                }
                ResourceLocal::Model(_) => {
                    self.models.entry(namespace).or_insert_with(HashSet::new).insert(selector);
                }
                ResourceLocal::Event(_) => {
                    self.events.entry(namespace).or_insert_with(HashSet::new).insert(selector);
                }
                ResourceLocal::Starknet(_) => {
                    self.starknet_contracts
                        .entry(namespace)
                        .or_insert_with(HashSet::new)
                        .insert(selector);
                }
                ResourceLocal::Namespace(_) => {
                    // Already processed earlier.
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starknet::core::types::contract::SierraClassDebugInfo;
    use starknet::core::types::EntryPointsByType;

    fn empty_sierra_class() -> SierraClass {
        SierraClass {
            abi: vec![],
            sierra_program: vec![],
            sierra_program_debug_info: SierraClassDebugInfo {
                type_names: vec![],
                libfunc_names: vec![],
                user_func_names: vec![],
            },
            contract_class_version: "0".to_string(),
            entry_points_by_type: EntryPointsByType {
                constructor: vec![],
                external: vec![],
                l1_handler: vec![],
            },
        }
    }

    #[test]
    fn test_add_resource() {
        let mut world = WorldLocal::new(NamespaceConfig::new("dojo"));

        assert_eq!(world.namespaces.len(), 1);
        assert_eq!(world.resources.len(), 1);

        let n = world.resources.get(&naming::compute_bytearray_hash("dojo")).unwrap();
        assert_eq!(n.name(), "dojo");

        world.add_resource(ResourceLocal::Contract(ContractLocal {
            name: "c1".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ZERO,
        }));

        let selector = naming::compute_selector_from_names(&"dojo".to_string(), &"c1".to_string());

        assert_eq!(world.contracts.get("dojo").unwrap().len(), 1);
        assert_eq!(world.contracts.get("dojo").unwrap().contains(&selector), true);
        assert_eq!(world.resources.len(), 1);

        world.add_resource(ResourceLocal::Contract(ContractLocal {
            name: "c2".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ZERO,
        }));

        let selector2 = naming::compute_selector_from_names(&"dojo".to_string(), &"c2".to_string());

        assert_eq!(world.contracts.get("dojo").unwrap().len(), 2);
        assert_eq!(world.resources.len(), 2);
        assert_eq!(world.contracts.get("dojo").unwrap().contains(&selector), true);
        assert_eq!(world.contracts.get("dojo").unwrap().contains(&selector2), true);
    }
}
