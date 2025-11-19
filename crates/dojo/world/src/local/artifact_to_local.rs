//! Converts Scarb artifacts to local resources.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Result, bail};
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use dojo_types::naming::{compute_bytearray_hash, compute_selector_from_names};
use serde_json;
use starknet::core::types::Felt;
use starknet::core::types::contract::{
    AbiEntry, AbiEvent, AbiImpl, AbiStruct, SierraClass, StateMutability,
    TypedAbiEvent,
};
use starknet::core::utils as snutils;
use starknet_crypto::poseidon_hash_many;
use tracing::{trace, warn};

use super::*;
use crate::config::ProfileConfig;
use crate::config::calldata_decoder::decode_calldata;

const WORLD_INTF: &str = "dojo::world::iworld::IWorld";
const CONTRACT_INTF: &str = "dojo::contract::interface::IContract";
const LIBRARY_INTF: &str = "dojo::contract::interface::ILibrary";
const MODEL_INTF: &str = "dojo::model::interface::IModel";
const EVENT_INTF: &str = "dojo::event::interface::IEvent";

#[derive(Debug, Clone)]
struct ExternalContractClassLocal {
    pub casm_class_hash: Felt,
    pub class: SierraClass,
    pub casm_class: Option<CasmContractClass>,
    pub entrypoints: Vec<String>,
    pub is_upgradeable: bool,
}

impl WorldLocal {
    pub fn from_directory<P: AsRef<Path>>(
        dir: P,
        profile_name: &str,
        profile_config: ProfileConfig,
    ) -> Result<Self> {
        trace!(
            ?profile_config,
            ?profile_name,
            directory = %dir.as_ref().to_string_lossy(),
            "Loading world from directory."
        );

        // Currently, we have no way to know the network chain-id from here.
        // We try to read the rpc_url of the config to infer the chain-id.
        let use_blake2s_class_hash = profile_config
            .env
            .as_ref()
            .map(|env| env.rpc_url.as_ref().unwrap().contains("sepolia") || env.rpc_url.as_ref().unwrap().contains("testnet"))
            .unwrap_or(false);

        trace!(
            use_blake2s_class_hash,
            "Using blake2s class hash for local world."
        );

        let mut resources = vec![];
        let mut external_contract_classes = HashMap::new();

        let mut world_class = None;
        let mut world_class_hash = None;
        let mut world_casm_class_hash = None;
        let mut world_casm_class = None;
        let mut world_entrypoints = vec![];

        if !dir.as_ref().exists() {
            return Err(anyhow::anyhow!(
                "Local Dojo world state not found for {profile_name} profile. Please build your \
                 project first with sozo build -P {profile_name}",
            ));
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if path.to_string_lossy().ends_with(".sierra.json") {
                    trace!("Ignored .sierra.json: {}", path.to_string_lossy().to_string());
                    continue;
                }

                if let Ok(sierra) =
                    serde_json::from_slice::<SierraClass>(std::fs::read(&path)?.as_slice())
                {
                    let casm_path = PathBuf::from(
                        path.to_string_lossy()
                            .to_string()
                            .replace("contract_class.json", "compiled_contract_class.json"),
                    );

                    let casm_class = if casm_path.exists() {
                        Some(serde_json::from_slice::<CasmContractClass>(
                            std::fs::read(&casm_path)?.as_slice(),
                        )?)
                    } else {
                        None
                    };

                    let abi = sierra.abi.clone();

                    let class_hash = sierra.class_hash()?;

                    // TODO: the casm must also use blake2s?
                    let casm_class_hash = casm_class_hash_from_sierra_file(&path, use_blake2s_class_hash)?;

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
                                world_casm_class = casm_class.clone();

                                trace!(
                                    class_hash = format!("{:#066x}", class_hash),
                                    casm_class_hash = format!("{:#066x}", casm_class_hash),
                                    "World adding world resource."
                                );

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
                                        class_hash = format!("{:#066x}", class_hash),
                                        casm_class_hash = format!("{:#066x}", casm_class_hash),
                                        "Adding local contract from artifact."
                                    );

                                    let resource = ResourceLocal::Contract(ContractLocal {
                                        common: CommonLocalInfo {
                                            namespace: ns.clone(),
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
                            ResourceType::Library(name) => {
                                let namespaces = profile_config.namespace.get_namespaces(&name);

                                let systems = systems_from_abi(&abi);

                                let mut added_resources = false;
                                for ns in namespaces {
                                    if let Some(version) = profile_config.lib_versions.as_ref() {
                                        if let Some(v) = version.get(&format!("{}-{}", ns, name)) {
                                            trace!(
                                                name,
                                                namespace = ns,
                                                version = v,
                                                class_hash = format!("{:#066x}", class_hash),
                                                casm_class_hash = format!("{:#066x}", casm_class_hash),
                                                "Adding local library from artifact."
                                            );

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
                                                version: v.clone(),
                                            });

                                            resources.push(resource);
                                            added_resources = true;
                                        }
                                    }
                                }

                                if !added_resources {
                                    warn!(
                                        "No library version found for library `{}` in the Dojo \
                                         profile config. Consider adding a `[lib_versions]` entry \
                                         with the version.",
                                        name
                                    );
                                } else {
                                    dojo_resource_found = true;
                                }

                                break;
                            }
                            ResourceType::Model(name) => {
                                let namespaces = profile_config.namespace.get_namespaces(&name);

                                for ns in namespaces {
                                    trace!(
                                        name,
                                        namespace = ns,
                                        class_hash = format!("{:#066x}", class_hash),
                                        casm_class_hash = format!("{:#066x}", casm_class_hash),
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
                                        members: members_from_abi(&abi),
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
                                        class_hash = format!("{:#066x}", class_hash),
                                        casm_class_hash = format!("{:#066x}", casm_class_hash),
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
                                        members: members_from_abi(&abi),
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
                            class_hash = format!("{:#066x}", class_hash),
                            casm_class_hash = format!("{:#066x}", casm_class_hash),
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

                        let entrypoints = systems_from_abi(&abi);

                        let is_upgradeable = is_upgradeable_from_abi(&abi);

                        external_contract_classes.insert(
                            contract_name.clone(),
                            ExternalContractClassLocal {
                                casm_class_hash,
                                class: sierra.clone(),
                                casm_class: casm_class.clone(),
                                entrypoints,
                                is_upgradeable,
                            },
                        );
                    }
                }
            }
        }

        if let Some(contracts) = &profile_config.external_contracts {
            for contract in contracts {
                if let Some(local_class) = external_contract_classes.get(&contract.contract_name) {
                    let encoded_constructor_data = if let Some(data) = &contract.constructor_data {
                        decode_calldata(data)?
                    } else {
                        vec![]
                    };

                    let instance_name =
                        contract.instance_name.clone().unwrap_or(contract.contract_name.clone());

                    let class_hash = local_class.class.class_hash()?;

                    let namespaces = profile_config.namespace.get_namespaces(&instance_name);

                    let salt = contract.salt.clone().unwrap_or("0".to_string());

                    for ns in namespaces {
                        let salt = poseidon_hash_many(&[
                            compute_selector_from_names(&ns, &instance_name),
                            compute_bytearray_hash(&salt),
                        ]);
                        let computed_address = snutils::get_contract_address(
                            salt,
                            class_hash,
                            &encoded_constructor_data,
                            Felt::ZERO,
                        );

                        let resource = ResourceLocal::ExternalContract(
                            ExternalContractLocal::SozoManaged(SozoManagedExternalContractLocal {
                                common: CommonLocalInfo {
                                    namespace: ns.clone(),
                                    name: instance_name.clone(),
                                    class: local_class.class.clone(),
                                    casm_class: local_class.casm_class.clone(),
                                    class_hash,
                                    casm_class_hash: local_class.casm_class_hash,
                                },
                                contract_name: contract.contract_name.clone(),
                                salt,
                                constructor_data: contract
                                    .constructor_data
                                    .clone()
                                    .unwrap_or(vec![]),
                                encoded_constructor_data: encoded_constructor_data.clone(),
                                computed_address,
                                entrypoints: local_class.entrypoints.clone(),
                                is_upgradeable: local_class.is_upgradeable,
                                block_number: contract.block_number,
                            }),
                        );

                        trace!(
                            contract_name = contract.contract_name.clone(),
                            instance_name = instance_name.clone(),
                            namespace = ns.clone(),
                            "Adding local external contract from artifact."
                        );

                        resources.push(resource);
                    }
                } else if let Some(contract_address) = &contract.contract_address {
                    let namespaces =
                        profile_config.namespace.get_namespaces(&contract.contract_name);

                    let contract_address = Felt::from_str(contract_address).unwrap_or_else(|_| {
                        panic!(
                            "Invalid format for {} contract_address field",
                            contract.contract_name
                        )
                    });

                    for ns in namespaces {
                        let resource = ResourceLocal::ExternalContract(
                            ExternalContractLocal::SelfManaged(SelfManagedExternalContractLocal {
                                name: contract.contract_name.clone(),
                                namespace: ns,
                                contract_address,
                                block_number: contract.block_number.unwrap_or(0),
                            }),
                        );

                        resources.push(resource);
                    }
                } else {
                    bail!(
                        "Your profile configuration mentions the external contract '{}' but it \
                         has NOT been compiled (sozo-managed) and no contract_address is \
                         specified (self-managed)",
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
fn casm_class_hash_from_sierra_file<P: AsRef<Path>>(path: P, use_blake2s_class_hash: bool) -> Result<Felt> {
    let bytecode_max_size = usize::MAX;
    let sierra_class: ContractClass =
        serde_json::from_slice::<ContractClass>(std::fs::read(path)?.as_slice())?;
    let casm_class =
        CasmContractClass::from_contract_class(sierra_class, false, bytecode_max_size)?;

    use starknet_api::contract_class::compiled_class_hash::{HashVersion, HashableCompiledClass};

    let hash_version = if use_blake2s_class_hash {
        HashVersion::V2
    } else {
        HashVersion::V1
    };

    let hash = casm_class.hash(&hash_version);

    Ok(Felt::from_bytes_be(
        &hash.0.to_bytes_be(),
    ))
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

/// Extracts the model or event events member from the ABI.
/// Since dojo is always adding an `ensure_abi` function to make sure the model or event
/// type is present in the ABI, we can use it to determine the model or event name
/// to after parse the struct from the ABI.
fn members_from_abi(abi: &[AbiEntry]) -> Vec<Member> {
    // Since the function comes after the struct or enum definition, we need to keep any struct
    // until we find the actual `ensure_abi` function.
    // And as we use the `Value` suffix to determine which members are values, we need to ensure
    // both ensure_abi and ensure_values are parsed.
    let mut structs: HashMap<String, AbiStruct> = HashMap::new();
    let mut model_or_event_name = None;
    let mut members = vec![];

    for entry in abi.iter() {
        match entry {
            AbiEntry::Struct(s) => {
                structs.insert(s.name.clone(), s.clone());
            }
            AbiEntry::Function(f) => {
                if f.name == "ensure_abi" {
                    let model_entry = f.inputs.first().expect("ensure_abi should have one input");

                    model_or_event_name = Some(model_entry.r#type.clone());
                }
            }
            _ => {}
        }
    }

    let name = model_or_event_name.expect("Model or event name not found");
    let value_name = format!("{name}Value");

    let struct_entry =
        structs.get(&name).unwrap_or_else(|| panic!("Struct not found in ABI: {name}"));

    let struct_entry_value = structs
        .get(&value_name)
        .unwrap_or_else(|| panic!("Struct value not found in ABI: {value_name}"));

    for member in &struct_entry.members {
        let is_value = struct_entry_value.members.iter().any(|m| m.name == member.name);

        members.push(Member {
            name: member.name.clone(),
            ty: member.r#type.clone(),
            key: !is_value,
        });
    }

    members
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

fn is_upgrade_fn(entry: &AbiEntry) -> bool {
    if let AbiEntry::Function(f) = entry {
        match f.state_mutability {
            StateMutability::External => {
                f.name == UPGRADE_CONTRACT_FN_NAME
                    && f.inputs.len() == 1
                    && f.inputs.first().unwrap().r#type == "core::starknet::class_hash::ClassHash"
            }
            _ => false,
        }
    } else {
        false
    }
}

fn is_upgradeable_from_abi(abi: &[AbiEntry]) -> bool {
    for entry in abi.iter() {
        if let AbiEntry::Interface(entry) = entry {
            if entry.items.iter().any(is_upgrade_fn) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use starknet::core::types::contract::{AbiFunction, AbiInterface, AbiNamedMember};

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

        let world = WorldLocal::from_directory(
            "../../../examples/simple/target/dev/",
            "dev",
            profile_config,
        )
        .unwrap();

        assert!(world.class_hash != Felt::ZERO);
        assert_eq!(world.resources.len(), 8);
        assert_eq!(
            world.entrypoints,
            vec![
                "uuid",
                "set_metadata",
                "register_namespace",
                "register_event",
                "register_model",
                "register_contract",
                "register_external_contract",
                "register_library",
                "init_contract",
                "upgrade_event",
                "upgrade_model",
                "upgrade_contract",
                "upgrade_external_contract",
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
        assert_eq!(
            systems,
            vec![
                "system_1",
                "system_2",
                "system_3",
                "system_4",
                "system_5",
                "upgrade",
                "system_free"
            ]
        );
    }

    #[test]
    fn test_is_upgradeable_from_abi() {
        fn build_abi(
            fname: &str,
            mtypes: Vec<&str>,
            state_mutability: StateMutability,
        ) -> Vec<AbiEntry> {
            [AbiEntry::Interface(AbiInterface {
                name: "IUpgrade".to_string(),
                items: [AbiEntry::Function(AbiFunction {
                    name: fname.to_string(),
                    inputs: mtypes
                        .iter()
                        .map(|t| AbiNamedMember {
                            name: "new_class_hash".to_string(),
                            r#type: t.to_string(),
                        })
                        .collect::<Vec<_>>(),
                    outputs: vec![],
                    state_mutability,
                })]
                .to_vec(),
            })]
            .to_vec()
        }

        assert!(
            is_upgradeable_from_abi(&build_abi(
                UPGRADE_CONTRACT_FN_NAME,
                vec!["core::starknet::class_hash::ClassHash"],
                StateMutability::External
            )),
            "Should be upgradeable"
        );

        assert!(!is_upgradeable_from_abi(&[]), "Should contain at least an interface");

        assert!(
            !is_upgradeable_from_abi(&build_abi(
                "upgrade_v2",
                vec!["core::starknet::class_hash::ClassHash"],
                StateMutability::External
            )),
            "Should be named 'upgrade'"
        );

        assert!(
            !is_upgradeable_from_abi(&build_abi(
                UPGRADE_CONTRACT_FN_NAME,
                vec!["u8"],
                StateMutability::External
            )),
            "Should have one parameter of type ClassHash"
        );

        assert!(
            !is_upgradeable_from_abi(&build_abi(
                UPGRADE_CONTRACT_FN_NAME,
                vec!["core::starknet::class_hash::ClassHash", "u8"],
                StateMutability::External
            )),
            "Should be have only one parameter"
        );

        assert!(
            !is_upgradeable_from_abi(&build_abi(
                UPGRADE_CONTRACT_FN_NAME,
                vec!["core::starknet::class_hash::ClassHash"],
                StateMutability::View
            )),
            "Should be external"
        );
    }
}
