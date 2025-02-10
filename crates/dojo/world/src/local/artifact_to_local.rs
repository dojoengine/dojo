//! Converts Scarb artifacts to local resources.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use dojo_types::naming::compute_bytearray_hash;
use serde_json;
use starknet::core::types::contract::{
    AbiEntry, AbiEvent, AbiImpl, CompiledClass, SierraClass, StateMutability, TypedAbiEvent,
};
use starknet::core::types::Felt;
use starknet::core::utils as snutils;
use starknet_crypto::poseidon_hash_many;
use tracing::trace;

use super::*;
use crate::config::calldata_decoder::decode_calldata;
use crate::config::ProfileConfig;

const WORLD_INTF: &str = "dojo::world::iworld::IWorld";
const CONTRACT_INTF: &str = "dojo::contract::interface::IContract";
const MODEL_INTF: &str = "dojo::model::interface::IModel";
const EVENT_INTF: &str = "dojo::event::interface::IEvent";

impl WorldLocal {
    pub fn from_directory<P: AsRef<Path>>(dir: P, profile_config: ProfileConfig) -> Result<Self> {
        trace!(
            ?profile_config,
            directory = %dir.as_ref().to_string_lossy(),
            "Loading world from directory."
        );
        let mut resources = vec![];
        let mut external_contract_classes = HashMap::new();

        let mut world_class = None;
        let mut world_class_hash = None;
        let mut world_casm_class_hash = None;
        let mut world_casm_class = None;
        let mut world_entrypoints = vec![];

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Ok(sierra) =
                    serde_json::from_reader::<_, SierraClass>(std::fs::File::open(&path)?)
                {
                    let casm_path = PathBuf::from(
                        path.to_string_lossy()
                            .to_string()
                            .replace("contract_class.json", "compiled_contract_class.json"),
                    );

                    let casm_class = if casm_path.exists() {
                        Some(serde_json::from_reader::<_, CompiledClass>(std::fs::File::open(
                            &casm_path,
                        )?)?)
                    } else {
                        None
                    };

                    let abi = sierra.abi.clone();
                    let class_hash = sierra.class_hash()?;
                    let casm_class_hash = casm_class_hash_from_sierra_file(&path)?;

                    let impls = abi
                        .iter()
                        .filter_map(|e| if let AbiEntry::Impl(i) = e { Some(i) } else { None })
                        .collect::<Vec<_>>();

                    // As a resource may be registered in multiple namespaces, currently the
                    // sierra class is being cloned for each namespace. Not ideal but keeping it
                    // simple for now.
                    let mut dojo_resource_found = false;

                    for i in impls {
                        match identify_resource_type(i) {
                            ResourceType::World => {
                                world_class = Some(sierra.clone());
                                world_class_hash = Some(class_hash);
                                world_casm_class_hash = Some(casm_class_hash);
                                world_entrypoints = systems_from_abi(&abi);
                                world_casm_class = casm_class;

                                dojo_resource_found = true;
                                break;
                            }
                            ResourceType::Contract(name) => {
                                let namespaces = profile_config.namespace.get_namespaces(&name);

                                let systems = systems_from_abi(&abi);

                                for ns in namespaces {
                                    trace!(
                                        name,
                                        namespace = ns,
                                        "Adding local contract from artifact."
                                    );

                                    let resource = ResourceLocal::Contract(ContractLocal {
                                        common: CommonLocalInfo {
                                            namespace: ns,
                                            name: name.clone(),
                                            class: sierra.clone(),
                                            casm_class: casm_class.clone(),
                                            class_hash,
                                            casm_class_hash,
                                        },
                                        systems: systems.clone(),
                                    });

                                    resources.push(resource);
                                }

                                dojo_resource_found = true;
                                break;
                            }
                            ResourceType::Model(name) => {
                                let namespaces = profile_config.namespace.get_namespaces(&name);

                                for ns in namespaces {
                                    trace!(
                                        name,
                                        namespace = ns,
                                        "Adding local model from artifact."
                                    );

                                    let resource = ResourceLocal::Model(ModelLocal {
                                        common: CommonLocalInfo {
                                            namespace: ns,
                                            name: name.clone(),
                                            class: sierra.clone(),
                                            casm_class: casm_class.clone(),
                                            class_hash,
                                            casm_class_hash,
                                        },
                                        members: vec![],
                                    });

                                    resources.push(resource);
                                }

                                dojo_resource_found = true;
                                break;
                            }
                            ResourceType::Event(name) => {
                                let namespaces = profile_config.namespace.get_namespaces(&name);

                                for ns in namespaces {
                                    trace!(
                                        name,
                                        namespace = ns,
                                        "Adding local event from artifact."
                                    );

                                    let resource = ResourceLocal::Event(EventLocal {
                                        common: CommonLocalInfo {
                                            namespace: ns,
                                            name: name.clone(),
                                            class: sierra.clone(),
                                            casm_class: casm_class.clone(),
                                            class_hash,
                                            casm_class_hash,
                                        },
                                        members: vec![],
                                    });

                                    resources.push(resource);
                                }

                                dojo_resource_found = true;
                                break;
                            }
                            ResourceType::Other => {}
                        }
                    }

                    // No Dojo resource found in this file so it is a classic Starknet contract
                    if !dojo_resource_found {
                        trace!(
                            filename = path.file_name().unwrap().to_str().unwrap(),
                            "Classic Starknet contract."
                        );

                        let contract_name = match contract_name_from_abi(&abi) {
                            Some(c) => c,
                            None => {
                                bail!(
                                    "Unable to find the name of the contract in the file {}",
                                    path.file_name().unwrap().to_str().unwrap()
                                );
                            }
                        };

                        external_contract_classes.insert(
                            contract_name.clone(),
                            ExternalContractClassLocal {
                                contract_name,
                                casm_class_hash,
                                class: sierra.clone(),
                            },
                        );
                    }
                }
            }
        }

        let mut external_contracts = vec![];

        if let Some(contracts) = &profile_config.external_contracts {
            for contract in contracts {
                if let Some(local_class) = external_contract_classes.get(&contract.contract_name) {
                    let raw_constructor_data = if let Some(data) = &contract.constructor_data {
                        decode_calldata(data)?
                    } else {
                        vec![]
                    };

                    let instance_name =
                        contract.instance_name.clone().unwrap_or(contract.contract_name.clone());

                    let salt = poseidon_hash_many(&[
                        compute_bytearray_hash(&instance_name),
                        compute_bytearray_hash(&contract.salt),
                    ]);
                    let class_hash = local_class.class.class_hash()?;

                    let address = snutils::get_contract_address(
                        salt,
                        class_hash,
                        &raw_constructor_data,
                        Felt::ZERO,
                    );

                    let instance = ExternalContractLocal {
                        contract_name: contract.contract_name.clone(),
                        class_hash,
                        instance_name,
                        salt,
                        constructor_data: contract.constructor_data.clone().unwrap_or(vec![]),
                        raw_constructor_data,
                        address,
                    };

                    trace!(
                        contract_name = contract.contract_name.clone(),
                        instance_name = instance.instance_name.clone(),
                        "External contract instance."
                    );

                    external_contracts.push(instance);
                } else {
                    bail!(
                        "Your profile configuration mentions the external contract '{}' but it \
                         has NOT been compiled.",
                        contract.contract_name
                    );
                }
            }
        }

        resources.push(ResourceLocal::Namespace(NamespaceLocal {
            name: profile_config.namespace.default.clone(),
        }));

        // Ensures all namespaces used as mapping key are registered as resources,
        // if it's not the default namespace.
        if let Some(mappings) = &profile_config.namespace.mappings {
            for ns in mappings.keys() {
                if ns != &profile_config.namespace.default {
                    resources.push(ResourceLocal::Namespace(NamespaceLocal { name: ns.clone() }));
                }
            }
        }

        let mut world = match (world_class, world_class_hash, world_casm_class_hash) {
            (Some(class), Some(class_hash), Some(casm_class_hash)) => Self {
                class,
                class_hash,
                casm_class: world_casm_class,
                casm_class_hash,
                resources: HashMap::new(),
                external_contract_classes,
                external_contracts,
                profile_config,
                entrypoints: world_entrypoints,
            },
            _ => {
                return Err(anyhow::anyhow!(
                    r#"
World artifact is missing, and required to deploy the world. Ensure you have \
added the contract to your Scarb.toml file:

[[target.starknet-contract]]
sierra = true
build-external-contracts = ["dojo::world::world_contract::world"]
"#
                ));
            }
        };

        for resource in resources {
            world.add_resource(resource);
        }

        Ok(world)
    }
}

/// Computes the casm class hash from a Sierra file path.
fn casm_class_hash_from_sierra_file<P: AsRef<Path>>(path: P) -> Result<Felt> {
    let bytecode_max_size = usize::MAX;
    let sierra_class: ContractClass =
        serde_json::from_reader::<_, ContractClass>(std::fs::File::open(path)?)?;
    let casm_class =
        CasmContractClass::from_contract_class(sierra_class, false, bytecode_max_size)?;
    Ok(casm_class.compiled_class_hash())
}

/// A simple enum to identify the type of resource with their name.
#[derive(Debug, PartialEq)]
enum ResourceType {
    World,
    Contract(String),
    Model(String),
    Event(String),
    Other,
}

/// Identifies the type of resource from the ABI implementation.
fn identify_resource_type(implem: &AbiImpl) -> ResourceType {
    if implem.interface_name == WORLD_INTF {
        ResourceType::World
    } else if implem.interface_name == CONTRACT_INTF {
        ResourceType::Contract(name_from_impl(&implem.name))
    } else if implem.interface_name == MODEL_INTF {
        ResourceType::Model(name_from_impl(&implem.name))
    } else if implem.interface_name == EVENT_INTF {
        ResourceType::Event(name_from_impl(&implem.name))
    } else {
        ResourceType::Other
    }
}

/// Extract the contract name from the `IContract`/`IModel`/`IEvent` implementation.
///
/// Dojo lang always output the implementation with the name of the contract itself, with
/// a double underscore as separator.
fn name_from_impl(impl_name: &str) -> String {
    impl_name.split("__").collect::<Vec<&str>>()[0].to_string()
}

fn systems_from_abi(abi: &[AbiEntry]) -> Vec<String> {
    fn extract_systems_from_abi_entry(entry: &AbiEntry) -> Vec<String> {
        match entry {
            AbiEntry::Function(f) => {
                if matches!(f.state_mutability, StateMutability::External) {
                    vec![f.name.clone()]
                } else {
                    vec![]
                }
            }
            AbiEntry::Interface(intf_entry) => {
                intf_entry.items.iter().flat_map(extract_systems_from_abi_entry).collect()
            }
            _ => vec![],
        }
    }

    abi.iter().flat_map(extract_systems_from_abi_entry).collect()
}

/// Get the contract name from the ABI.
///
/// Note: The last AbiEntry of type `event` and kind `enum` is always the main
/// Event enum of the contract. So, we find it and use its name to get
/// the contract name.
fn contract_name_from_abi(abi: &[AbiEntry]) -> Option<String> {
    for entry in abi.iter().rev() {
        if let AbiEntry::Event(AbiEvent::Typed(TypedAbiEvent::Enum(e))) = entry {
            let mut it = e.name.rsplit("::");
            if let (Some(_), Some(name)) = (it.next(), it.next()) {
                return Some(name.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NamespaceConfig;

    #[test]
    fn test_name_from_impl() {
        assert_eq!(name_from_impl("contract__MyImpl"), "contract");
        assert_eq!(name_from_impl("Model__MyModel"), "Model");
        assert_eq!(name_from_impl("Event__MyEvent"), "Event");
    }

    #[test]
    fn test_identify_resource_type() {
        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: WORLD_INTF.to_string(),
                name: "IWorld".to_string()
            }),
            ResourceType::World
        );

        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: CONTRACT_INTF.to_string(),
                name: "contract__DojoModelImpl".to_string()
            }),
            ResourceType::Contract("contract".to_string())
        );

        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: MODEL_INTF.to_string(),
                name: "Model__DojoModelImpl".to_string()
            }),
            ResourceType::Model("Model".to_string())
        );

        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: EVENT_INTF.to_string(),
                name: "Event__DojoEventImpl".to_string()
            }),
            ResourceType::Event("Event".to_string())
        );
    }

    #[test]
    fn test_load_world_from_directory() {
        let namespace_config = NamespaceConfig::new("dojo");
        let profile_config = ProfileConfig::new("test", "seed", namespace_config);

        let world =
            WorldLocal::from_directory("../../../examples/simple/target/dev/", profile_config)
                .unwrap();

        assert!(world.class_hash != Felt::ZERO);
        assert_eq!(world.resources.len(), 7);
        assert_eq!(
            world.entrypoints,
            vec![
                "uuid",
                "set_metadata",
                "register_namespace",
                "register_event",
                "register_model",
                "register_contract",
                "init_contract",
                "upgrade_event",
                "upgrade_model",
                "upgrade_contract",
                "emit_event",
                "emit_events",
                "set_entity",
                "set_entities",
                "delete_entity",
                "delete_entities",
                "grant_owner",
                "revoke_owner",
                "grant_writer",
                "revoke_writer",
                "upgrade"
            ]
        );
    }

    #[test]
    fn test_systems_from_abi() {
        let abi = serde_json::from_reader::<_, SierraClass>(
            std::fs::File::open(
                "../../../examples/simple/target/dev/dojo_simple_c1.contract_class.json",
            )
            .unwrap(),
        )
        .unwrap();

        let systems = systems_from_abi(&abi.abi);
        assert_eq!(systems, vec!["system_1", "system_2", "system_3", "system_4", "upgrade"]);
    }
}
