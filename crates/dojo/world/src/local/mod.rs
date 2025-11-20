//! Local resources for the world, gathered from the Scarb generated artifacts.
//!
//! When resources are compiled, there's no namespace attached to them.
//! However, to be registered and used in the world, they need to be namespaced.
//! To link a local resource to its world representation, a namespace configuration
//! is needed.
//!
//! Class hashes are cached into the resource to avoid recomputing them when
//! requesting it.

use std::collections::HashMap;

use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use dojo_types::naming;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::Felt;
use starknet::core::utils::CairoShortStringToFeltError;

mod artifact_to_local;
mod resource;

pub use resource::*;

use crate::config::ProfileConfig;
use crate::utils::compute_world_address;
use crate::{ContractAddress, DojoSelector};

pub const UPGRADE_CONTRACT_FN_NAME: &str = "upgrade";

#[derive(Debug, Clone)]
pub struct WorldLocal {
    /// The class of the world.
    pub class: SierraClass,
    /// The class hash of the world.
    pub class_hash: Felt,
    /// The casm class of the world.
    pub casm_class: Option<CasmContractClass>,
    /// The casm class hash of the world.
    pub casm_class_hash: Felt,
    /// The resources of the world.
    pub resources: HashMap<DojoSelector, ResourceLocal>,
    /// The profile configuration of the local world.
    pub profile_config: ProfileConfig,
    /// All the entrypoints that are exposed by the world
    /// and can be targeted by a transaction.
    pub entrypoints: Vec<String>,
}

#[cfg(test)]
impl Default for WorldLocal {
    fn default() -> Self {
        use starknet::core::types::contract::{SierraClass, SierraClassDebugInfo};
        use starknet::core::types::EntryPointsByType;

        Self {
            class: SierraClass {
                sierra_program: Vec::new(),
                sierra_program_debug_info: SierraClassDebugInfo {
                    type_names: Vec::new(),
                    libfunc_names: Vec::new(),
                    user_func_names: Vec::new(),
                },
                contract_class_version: "".to_string(),
                entry_points_by_type: EntryPointsByType {
                    constructor: Vec::new(),
                    external: Vec::new(),
                    l1_handler: Vec::new(),
                },
                abi: Vec::new(),
            },
            casm_class: None,
            class_hash: Felt::ZERO,
            casm_class_hash: Felt::ZERO,
            resources: HashMap::new(),
            profile_config: ProfileConfig::default(),
            entrypoints: vec![],
        }
    }
}

impl WorldLocal {
    #[cfg(test)]
    /// Initializes a new world with namespaces from the profile configuration.
    pub fn new(profile_config: ProfileConfig) -> Self {
        let mut world = Self { profile_config: profile_config.clone(), ..Default::default() };

        world.add_resource(ResourceLocal::Namespace(NamespaceLocal {
            name: profile_config.namespace.default,
        }));

        if let Some(mappings) = &profile_config.namespace.mappings {
            for ns in mappings.keys() {
                world.add_resource(ResourceLocal::Namespace(NamespaceLocal { name: ns.clone() }));
            }
        }

        world
    }

    /// Computes the deterministic address of the world contract.
    pub fn deterministic_world_address(&self) -> Result<Felt, CairoShortStringToFeltError> {
        let class_hash = self.class_hash;
        compute_world_address(&self.profile_config.world.seed, class_hash)
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

    /// Returns the contract resource, if any.
    pub fn get_contract_resource(&self, selector: DojoSelector) -> Option<&ContractLocal> {
        self.resources.get(&selector).and_then(|r| r.as_contract())
    }

    /// Gets the deterministic contract address only based on local information.
    pub fn get_contract_address_local(&self, selector: DojoSelector) -> Option<ContractAddress> {
        let contract = self.get_contract_resource(selector)?;
        Some(crate::utils::compute_dojo_contract_address(
            selector,
            contract.common.class_hash,
            self.deterministic_world_address().unwrap(),
        ))
    }

    /// Returns the resource from a name or tag.
    pub fn resource_from_name_or_tag(&self, name_or_tag: &str) -> Option<&ResourceLocal> {
        let selector = if naming::is_valid_tag(name_or_tag) {
            naming::compute_selector_from_tag(name_or_tag)
        } else {
            naming::compute_bytearray_hash(name_or_tag)
        };

        self.resources.get(&selector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NamespaceConfig;
    use crate::test_utils::empty_sierra_class;

    #[test]
    fn test_add_resource() {
        let profile_config = ProfileConfig::new("test", "seed", NamespaceConfig::new("dojo"));
        let mut world = WorldLocal::new(profile_config);

        assert_eq!(world.resources.len(), 1);

        let n = world.resources.get(&naming::compute_bytearray_hash("dojo")).unwrap();
        assert_eq!(n.name(), "dojo");

        world.add_resource(ResourceLocal::Contract(ContractLocal {
            common: CommonLocalInfo {
                name: "c1".to_string(),
                namespace: "dojo".to_string(),
                class: empty_sierra_class(),
                casm_class: None,
                class_hash: Felt::ZERO,
                casm_class_hash: Felt::ZERO,
            },
            systems: vec![],
        }));

        let selector = naming::compute_selector_from_names("dojo", "c1");

        assert_eq!(world.resources.len(), 2);
        assert!(world.get_contract_resource(selector).is_some());

        world.add_resource(ResourceLocal::Contract(ContractLocal {
            common: CommonLocalInfo {
                name: "c2".to_string(),
                namespace: "dojo".to_string(),
                class: empty_sierra_class(),
                casm_class: None,
                class_hash: Felt::ZERO,
                casm_class_hash: Felt::ZERO,
            },
            systems: vec![],
        }));

        let selector2 = naming::compute_selector_from_names("dojo", "c2");

        assert_eq!(world.resources.len(), 3);
        assert!(world.get_contract_resource(selector).is_some());
        assert!(world.get_contract_resource(selector2).is_some());
    }
}
