//! A contract info type used to abstract from where the deterministic
//! contracts address comes from.
//!
//! This also embeds the ABI of the contract to ensure easy access to it.
//! To illustrate the use case, when a manifest has been generated locally,
//! it's faster to load information from it to for instance use `sozo execute`
//! instead of fetching the information from the network.
//!
//! However, in some situations, the manifest may be outdated and some contracts may
//! be missing since the manifest is only generated after a migration. In this situation,
//! Sozo is fetching all the contracts information by comparing the local world and the
//! chain state (using the `world diff`).
//!
//! We could have used the local world, but if the world has been migrated, the address
//! of the contracts will have changed since the original class hash of the world is not
//! present locally. Only onchain.
use std::collections::HashMap;

use starknet::core::types::Felt;
use tracing::trace;

use crate::diff::{Manifest, ResourceDiff, WorldDiff};
use crate::local::ResourceLocal;
use crate::remote::ResourceRemote;

#[derive(Debug, PartialEq)]
pub struct ContractInfo {
    /// Tag of the contract (or world).
    pub tag_or_name: String,
    /// The address of the contract.
    pub address: Felt,
    /// The entrypoints that can be targeted with a transaction.
    /// This only includes `external` functions.
    pub entrypoints: Vec<String>,
}

impl From<&Manifest> for HashMap<String, ContractInfo> {
    fn from(manifest: &Manifest) -> Self {
        trace!("Converting manifest to contracts info.");
        let mut contracts = HashMap::new();

        contracts.insert(
            "world".to_string(),
            ContractInfo {
                tag_or_name: "world".to_string(),
                address: manifest.world.address,
                entrypoints: manifest.world.entrypoints.clone(),
            },
        );

        for c in &manifest.contracts {
            contracts.insert(
                c.tag.clone(),
                ContractInfo {
                    tag_or_name: c.tag.clone(),
                    address: c.address,
                    entrypoints: c.systems.clone(),
                },
            );
        }

        for c in &manifest.external_contracts {
            contracts.insert(
                c.tag.clone(),
                ContractInfo {
                    tag_or_name: c.tag.clone(),
                    address: c.address,
                    entrypoints: vec![],
                },
            );
        }

        contracts
    }
}

impl From<&WorldDiff> for HashMap<String, ContractInfo> {
    fn from(world_diff: &WorldDiff) -> Self {
        trace!("Converting world diff to contracts info.");
        let mut contracts = HashMap::new();

        contracts.insert(
            "world".to_string(),
            ContractInfo {
                tag_or_name: "world".to_string(),
                address: world_diff.world_info.address,
                entrypoints: world_diff.world_info.entrypoints.clone(),
            },
        );

        for (selector, resource) in &world_diff.resources {
            let tag = resource.tag();

            match resource {
                ResourceDiff::Created(ResourceLocal::Contract(c)) => {
                    // The resource must exist, so the unwrap is safe here.
                    let address = world_diff.get_contract_address(*selector).unwrap();
                    contracts.insert(
                        tag.clone(),
                        ContractInfo {
                            tag_or_name: tag.clone(),
                            address,
                            entrypoints: c.systems.clone(),
                        },
                    );
                }
                ResourceDiff::Updated(ResourceLocal::Contract(l), ResourceRemote::Contract(r))
                | ResourceDiff::Synced(ResourceLocal::Contract(l), ResourceRemote::Contract(r)) => {
                    contracts.insert(
                        tag.clone(),
                        ContractInfo {
                            tag_or_name: tag.clone(),
                            address: r.common.address,
                            entrypoints: l.systems.clone(),
                        },
                    );
                }
                ResourceDiff::Created(ResourceLocal::ExternalContract(l)) => {
                    contracts.insert(
                        tag.clone(),
                        ContractInfo {
                            tag_or_name: tag.clone(),
                            address: l.computed_address,
                            entrypoints: l.entrypoints.clone(),
                        },
                    );
                }
                ResourceDiff::Updated(
                    ResourceLocal::ExternalContract(l),
                    ResourceRemote::ExternalContract(r),
                )
                | ResourceDiff::Synced(
                    ResourceLocal::ExternalContract(l),
                    ResourceRemote::ExternalContract(r),
                ) => {
                    contracts.insert(
                        tag.clone(),
                        ContractInfo {
                            tag_or_name: tag.clone(),
                            address: if l.is_upgradeable {
                                r.common.address
                            } else {
                                l.computed_address
                            },
                            entrypoints: l.entrypoints.clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        contracts
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::contract::{SierraClass, SierraClassDebugInfo};
    use starknet::core::types::EntryPointsByType;
    use starknet::macros::felt;

    use super::*;
    use crate::diff::{DojoContract, DojoLibrary, DojoModel, WorldContract};
    use crate::local::{CommonLocalInfo, ContractLocal, WorldLocal};

    #[test]
    fn test_manifest_to_contracts_info() {
        let manifest = Manifest {
            world: WorldContract {
                address: felt!("0x5678"),
                class_hash: felt!("0x1111"),
                seed: "test_seed".to_string(),
                name: "test_world".to_string(),
                entrypoints: vec!["execute".to_string()],
                abi: vec![],
            },
            contracts: vec![DojoContract {
                address: felt!("0x1234"),
                class_hash: felt!("0x2222"),
                abi: vec![],
                init_calldata: vec![],
                tag: "ns-test_contract".to_string(),
                systems: vec!["system_1".to_string()],
                selector: felt!("0x3333"),
            }],
            libraries: vec![DojoLibrary {
                class_hash: felt!("0x9999"),
                abi: vec![],
                tag: "ns-test_library".to_string(),
                systems: vec!["system_1".to_string()],
                selector: felt!("0x999"),
                version: "0.0.0".to_string(),
            }],
            models: vec![DojoModel {
                tag: "ns-test_model".to_string(),
                class_hash: felt!("0x4444"),
                members: vec![],
                selector: felt!("0x5555"),
            }],
            events: vec![],
            external_contracts: vec![],
        };

        let contracts_info: HashMap<String, ContractInfo> = (&manifest).into();
        assert_eq!(contracts_info.len(), 2);
        assert_eq!(contracts_info["world"].address, felt!("0x5678"));
        assert_eq!(contracts_info["world"].entrypoints, vec!["execute".to_string()]);
        assert_eq!(contracts_info["ns-test_contract"].address, felt!("0x1234"));
        assert_eq!(contracts_info["ns-test_contract"].entrypoints, vec!["system_1".to_string()]);
        assert_eq!(contracts_info["ns-test_contract"].tag_or_name, "ns-test_contract".to_string());
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_world_diff_to_contracts_info() {
        let mut local = WorldLocal::default();
        local.entrypoints = vec!["execute".to_string()];

        let contract = ContractLocal {
            common: CommonLocalInfo {
                name: "test_contract".to_string(),
                namespace: "ns".to_string(),
                class: SierraClass {
                    sierra_program: vec![],
                    sierra_program_debug_info: SierraClassDebugInfo {
                        type_names: vec![],
                        libfunc_names: vec![],
                        user_func_names: vec![],
                    },
                    contract_class_version: "".to_string(),
                    entry_points_by_type: EntryPointsByType {
                        constructor: vec![],
                        external: vec![],
                        l1_handler: vec![],
                    },
                    abi: vec![],
                },
                casm_class: None,
                class_hash: felt!("0x2222"),
                casm_class_hash: felt!("0x2222"),
            },
            systems: vec!["system_1".to_string()],
        };

        local.profile_config.namespace.default = "ns".to_string();
        local.add_resource(ResourceLocal::Contract(contract));

        let world_diff = WorldDiff::from_local(local).unwrap();

        let contracts_info: HashMap<String, ContractInfo> = (&world_diff).into();
        assert_eq!(contracts_info.len(), 4);
        assert_eq!(
            contracts_info["world"].address,
            felt!("0x66c1fe28a8f6c5f1dfe797df547fb683d1c9d18c87b049021f115f026be8077")
        );
        assert_eq!(contracts_info["world"].entrypoints, vec!["execute".to_string()]);
        assert_eq!(
            contracts_info["ns-test_contract"].address,
            felt!("0x2a03d1761c3e0ee912794d32d5f9be9ae7d1af0fc349fc040fe292a096785ad")
        );
        assert_eq!(contracts_info["ns-test_contract"].entrypoints, vec!["system_1".to_string()]);
        assert_eq!(contracts_info["ns-test_contract"].tag_or_name, "ns-test_contract".to_string());

        assert_eq!(
            contracts_info["Instance1"],
            ContractInfo {
                tag_or_name: "Instance1".to_string(),
                address: Felt::from_hex("0x6789").unwrap(),
                entrypoints: vec![]
            }
        );
        assert_eq!(
            contracts_info["Instance2"],
            ContractInfo {
                tag_or_name: "Instance2".to_string(),
                address: Felt::from_hex("0x1234").unwrap(),
                entrypoints: vec![]
            }
        );
    }
}
