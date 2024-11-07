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

#[derive(Debug)]
pub struct ContractInfo {
    /// Tag of the contract (or world).
    pub tag: String,
    /// The address of the contract.
    pub address: Felt,
    /// The entrypoints that can be targetted with a transaction.
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
                tag: "world".to_string(),
                address: manifest.world.address,
                entrypoints: manifest.world.entrypoints.clone(),
            },
        );

        for c in &manifest.contracts {
            contracts.insert(
                c.tag.clone(),
                ContractInfo {
                    tag: c.tag.clone(),
                    address: c.address,
                    entrypoints: c.systems.clone(),
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
                tag: "world".to_string(),
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
                        ContractInfo { tag: tag.clone(), address, entrypoints: c.systems.clone() },
                    );
                }
                ResourceDiff::Updated(ResourceLocal::Contract(l), ResourceRemote::Contract(r)) => {
                    contracts.insert(
                        tag.clone(),
                        ContractInfo {
                            tag: tag.clone(),
                            address: r.common.address,
                            entrypoints: l.systems.clone(),
                        },
                    );
                }
                ResourceDiff::Synced(ResourceLocal::Contract(l), ResourceRemote::Contract(r)) => {
                    contracts.insert(
                        tag.clone(),
                        ContractInfo {
                            tag: tag.clone(),
                            address: r.common.address,
                            entrypoints: l.systems.clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        contracts
    }
}
