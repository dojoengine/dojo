//! Converts Scarb artifacts to local resources.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use serde_json;
use starknet::core::types::contract::{
    AbiEntry, AbiImpl, CompiledClass, SierraClass, StateMutability,
};
use starknet::core::types::Felt;
use tracing::trace;

use super::*;
use crate::config::ProfileConfig;

const WORLD_INTF: &str = "dojo::world::iworld::IWorld";
const CONTRACT_INTF: &str = "dojo::contract::interface::IContract";
const LIBRARY_INTF: &str = "dojo::contract::interface::ILibrary";
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
                    for i in impls {
                        match identify_resource_type(i) {
                            ResourceType::World => {
                                world_class = Some(sierra);
                                world_class_hash = Some(class_hash);
                                world_casm_class_hash = Some(casm_class_hash);
                                world_entrypoints = systems_from_abi(&abi);
                                world_casm_class = casm_class;
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
                                break;
                            }
                            ResourceType::Library(name) => {
                                let namespaces = profile_config.namespace.get_namespaces(&name);

                                let systems = systems_from_abi(&abi);

                                for ns in namespaces {
                                    trace!(
                                        name,
                                        namespace = ns,
                                        "Adding local library from artifact."
                                    );

                                    let version = profile_config
                                        .lib_versions
                                        .as_ref()
                                        .expect("expected lib version a")
                                        .get(&format!("{}-{}", ns, name))
                                        .expect("expected lib version b");

                                    let resource = ResourceLocal::Library(LibraryLocal {
                                        common: CommonLocalInfo {
                                            namespace: ns,
                                            name: name.clone(),
                                            class: sierra.clone(),
                                            casm_class: casm_class.clone(),
                                            class_hash,
                                            casm_class_hash,
                                        },
                                        systems: systems.clone(),
                                        version: version.clone(),
                                    });

                                    resources.push(resource);
                                }
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
                                break;
                            }
                            ResourceType::Other => {}
                        }
                    }
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
    Library(String),
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
    } else if implem.interface_name == LIBRARY_INTF {
        ResourceType::Library(name_from_impl(&implem.name))
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
