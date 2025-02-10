//! Manifest data to store the diff result in files.

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::contract::AbiEntry;
use starknet::core::types::Felt;

use super::{ResourceDiff, WorldDiff};
use crate::local::ResourceLocal;
use crate::remote::ResourceRemote;
use crate::ResourceType;

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub world: WorldContract,
    pub contracts: Vec<DojoContract>,
    pub models: Vec<DojoModel>,
    pub events: Vec<DojoEvent>,
    pub external_contracts: Vec<ExternalContract>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct WorldContract {
    /// Class hash of the contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    /// Address of the contract.
    #[serde_as(as = "UfeHex")]
    pub address: Felt,
    /// Seed used to deploy the world.
    pub seed: String,
    /// Name of the world.
    pub name: String,
    /// Entrypoints of the world.
    pub entrypoints: Vec<String>,
    /// Abi of the world.
    pub abi: Vec<AbiEntry>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct DojoContract {
    /// Address of the contract.
    #[serde_as(as = "UfeHex")]
    pub address: Felt,
    /// Class hash of the contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    /// ABI of the contract.
    pub abi: Vec<AbiEntry>,
    /// Initialization call data.
    #[serde(default)]
    pub init_calldata: Vec<String>,
    /// Tag of the contract.
    pub tag: String,
    /// Selector of the contract.
    #[serde_as(as = "UfeHex")]
    pub selector: Felt,
    /// Systems of the contract.
    pub systems: Vec<String>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct DojoModel {
    /// Members of the model.
    pub members: Vec<Member>,
    /// Class hash of the model.
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    /// Tag of the model.
    pub tag: String,
    /// Selector of the model.
    #[serde_as(as = "UfeHex")]
    pub selector: Felt,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct DojoEvent {
    /// Members of the event.
    pub members: Vec<Member>,
    /// Class hash of the event.
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    /// Tag of the event.
    pub tag: String,
    /// Selector of the event.
    #[serde_as(as = "UfeHex")]
    pub selector: Felt,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct ExternalContract {
    /// Class hash of the contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    /// Contract Name
    pub contract_name: String,
    /// Instance name
    pub instance_name: String,
    /// Contract address
    #[serde_as(as = "UfeHex")]
    pub address: Felt,
    /// ABI of the contract.
    pub abi: Vec<AbiEntry>,
    /// Initialization call data.
    #[serde(default)]
    pub constructor_calldata: Vec<String>,
}

/// Represents a model member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    /// Name of the member.
    pub name: String,
    /// Type of the member.
    #[serde(rename = "type")]
    pub ty: String,
    /// Whether the member is a key.
    pub key: bool,
}

impl Manifest {
    pub fn new(diff: &WorldDiff) -> Self {
        let world = WorldContract {
            class_hash: diff.world_info.class_hash,
            address: diff.world_info.address,
            seed: diff.profile_config.world.seed.clone(),
            name: diff.profile_config.world.name.clone(),
            entrypoints: diff.world_info.entrypoints.clone(),
            abi: diff.world_info.class.abi.clone(),
        };

        let mut contracts = Vec::new();
        let mut models = Vec::new();
        let mut events = Vec::new();
        let mut external_contracts = Vec::new();

        for resource in diff.resources.values() {
            if diff.profile_config.is_skipped(&resource.tag()) {
                continue;
            }

            match resource.resource_type() {
                ResourceType::Contract => {
                    contracts.push(resource_diff_to_dojo_contract(diff, resource))
                }
                ResourceType::Model => models.push(resource_diff_to_dojo_model(resource)),
                ResourceType::Event => events.push(resource_diff_to_dojo_event(resource)),
                ResourceType::Namespace => {}
                ResourceType::StarknetContract => {}
            }
        }

        for contract in diff.external_contracts.values() {
            let contract = contract.contract_data();

            external_contracts.push(ExternalContract {
                contract_name: contract.contract_name.clone(),
                instance_name: contract.instance_name,
                class_hash: contract.class_hash,
                address: contract.address,
                constructor_calldata: contract.constructor_data,
                abi: diff
                    .external_contract_classes
                    .get(&contract.contract_name)
                    .map_or(vec![], |d| d.class_data().class.abi),
            })
        }

        // Keep order to ensure deterministic output.
        contracts.sort_by_key(|c| c.tag.clone());
        models.sort_by_key(|m| m.tag.clone());
        events.sort_by_key(|e| e.tag.clone());
        external_contracts.sort_by_key(|c| c.instance_name.clone());

        Self { world, contracts, models, events, external_contracts }
    }

    pub fn get_contract_address(&self, tag: &str) -> Option<Felt> {
        self.contracts.iter().find_map(|c| if c.tag == tag { Some(c.address) } else { None })
    }
}

fn resource_diff_to_dojo_contract(diff: &WorldDiff, resource: &ResourceDiff) -> DojoContract {
    let init_calldata = if let Some(init_call_args) = &diff.profile_config.init_call_args {
        init_call_args.get(&resource.tag()).unwrap_or(&vec![]).clone()
    } else {
        vec![]
    };

    let tag = resource.tag();

    match &resource {
        ResourceDiff::Created(ResourceLocal::Contract(l)) => DojoContract {
            address: diff.get_contract_address(resource.dojo_selector()).unwrap(),
            class_hash: l.common.class_hash,
            abi: l.common.class.abi.clone(),
            init_calldata,
            tag,
            systems: l.systems.clone(),
            selector: resource.dojo_selector(),
        },
        ResourceDiff::Updated(ResourceLocal::Contract(l), ResourceRemote::Contract(r))
        | ResourceDiff::Synced(ResourceLocal::Contract(l), ResourceRemote::Contract(r)) => {
            DojoContract {
                address: r.common.address,
                class_hash: l.common.class_hash,
                abi: l.common.class.abi.clone(),
                init_calldata,
                tag,
                systems: l.systems.clone(),
                selector: resource.dojo_selector(),
            }
        }
        _ => unreachable!(),
    }
}

fn resource_diff_to_dojo_model(resource: &ResourceDiff) -> DojoModel {
    let tag = resource.tag();

    match &resource {
        ResourceDiff::Created(ResourceLocal::Model(l)) => DojoModel {
            members: l
                .members
                .iter()
                .map(|m| Member { name: m.name.clone(), ty: m.ty.clone(), key: m.key })
                .collect(),
            class_hash: l.common.class_hash,
            tag,
            selector: resource.dojo_selector(),
        },
        ResourceDiff::Updated(ResourceLocal::Model(l), _)
        | ResourceDiff::Synced(ResourceLocal::Model(l), _) => DojoModel {
            members: l
                .members
                .iter()
                .map(|m| Member { name: m.name.clone(), ty: m.ty.clone(), key: m.key })
                .collect(),
            class_hash: l.common.class_hash,
            tag,
            selector: resource.dojo_selector(),
        },
        _ => unreachable!(),
    }
}

fn resource_diff_to_dojo_event(resource: &ResourceDiff) -> DojoEvent {
    let tag = resource.tag();

    match &resource {
        ResourceDiff::Created(ResourceLocal::Event(l)) => DojoEvent {
            members: l
                .members
                .iter()
                .map(|m| Member { name: m.name.clone(), ty: m.ty.clone(), key: m.key })
                .collect(),
            class_hash: l.common.class_hash,
            tag,
            selector: resource.dojo_selector(),
        },
        ResourceDiff::Updated(ResourceLocal::Event(l), _)
        | ResourceDiff::Synced(ResourceLocal::Event(l), _) => DojoEvent {
            members: l
                .members
                .iter()
                .map(|m| Member { name: m.name.clone(), ty: m.ty.clone(), key: m.key })
                .collect(),
            class_hash: l.common.class_hash,
            tag,
            selector: resource.dojo_selector(),
        },
        _ => unreachable!(),
    }
}
