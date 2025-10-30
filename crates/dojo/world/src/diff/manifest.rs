//! Manifest data to store the diff result in files.

use std::collections::HashMap;

use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::contract::{AbiEntry, AbiEvent, TypedAbiEvent, UntypedAbiEvent};
use starknet::core::types::Felt;

use super::{ResourceDiff, WorldDiff};
use crate::local::{ExternalContractLocal, ResourceLocal};
use crate::remote::ResourceRemote;
use crate::ResourceType;

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub world: WorldContract,
    pub contracts: Vec<DojoContract>,
    pub libraries: Vec<DojoLibrary>,
    pub models: Vec<DojoModel>,
    pub events: Vec<DojoEvent>,
    pub external_contracts: Vec<ExternalContract>,
    /// The all in one ABIs without duplicates, but keep the original `AbiEntry` serialization as a
    /// vector. When serialized, the entries are always sorted alphabetically by name.
    #[serde(
        serialize_with = "serialize_abis_hashmap",
        deserialize_with = "deserialize_abis_hashmap"
    )]
    pub abis: HashMap<String, AbiEntry>,
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
    /// Abi of the world, skipped during serialization in favor of the `abis` field.
    #[serde(skip)]
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
    /// ABI of the contract, skipped during serialization in favor of the `abis` field.
    #[serde(skip)]
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
pub struct DojoLibrary {
    /// Class hash of the contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    /// ABI of the contract, skipped during serialization in favor of the `abis` field.
    #[serde(skip)]
    pub abi: Vec<AbiEntry>,
    /// Tag of the contract.
    pub tag: String,
    /// Selector of the contract.
    #[serde_as(as = "UfeHex")]
    pub selector: Felt,
    /// Systems of the library.
    pub systems: Vec<String>,
    /// Version of the library
    pub version: String,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
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
    /// ABI of the model, skipped during serialization in favor of the `abis` field.
    #[serde(skip)]
    pub abi: Vec<AbiEntry>,
}

#[cfg(test)]
impl PartialEq for DojoModel {
    fn eq(&self, other: &Self) -> bool {
        self.members == other.members
            && self.class_hash == other.class_hash
            && self.tag == other.tag
            && self.selector == other.selector
            && self.abi.len() == other.abi.len()
    }
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
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
    /// ABI of the event, skipped during serialization in favor of the `abis` field.
    #[serde(skip)]
    pub abi: Vec<AbiEntry>,
}

#[cfg(test)]
impl PartialEq for DojoEvent {
    fn eq(&self, other: &Self) -> bool {
        self.members == other.members
            && self.class_hash == other.class_hash
            && self.tag == other.tag
            && self.selector == other.selector
            && self.abi.len() == other.abi.len()
    }
}
#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct ExternalContract {
    /// Class hash of the contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    /// Contract Name
    pub contract_name: String,
    /// Tag of the external contract.
    pub tag: String,
    /// Contract address
    #[serde_as(as = "UfeHex")]
    pub address: Felt,
    /// ABI of the contract.
    /// Ignored during serialization in favor of the `abis` field.
    #[serde(skip)]
    pub abi: Vec<AbiEntry>,
    /// Human-readeable constructor call data.
    #[serde(default)]
    pub constructor_calldata: Vec<String>,
    /// Encoded constructor call data.
    #[serde(default)]
    pub encoded_constructor_calldata: Vec<Felt>,
    /// Entry points of the contract.
    pub entrypoints: Vec<String>,
    /// Block number used for indexing.
    pub block_number: Option<u64>,
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
        let mut libraries = Vec::new();
        let mut external_contracts = Vec::new();
        let mut abis = HashMap::new();

        add_abi_entries(&mut abis, diff.world_info.class.abi.clone());

        for resource in diff.resources.values() {
            if diff.profile_config.is_skipped(&resource.tag()) {
                continue;
            }

            match resource.resource_type() {
                ResourceType::Contract => {
                    contracts.push(resource_diff_to_dojo_contract(diff, resource))
                }
                ResourceType::Library => {
                    libraries.push(resource_diff_to_dojo_library(diff, resource))
                }
                ResourceType::Model => models.push(resource_diff_to_dojo_model(resource)),
                ResourceType::Event => events.push(resource_diff_to_dojo_event(resource)),
                ResourceType::Namespace => {}
                ResourceType::ExternalContract => {
                    external_contracts.push(resource_diff_to_dojo_external_contract(resource))
                }
            }
        }

        // Keep order to ensure deterministic output.
        contracts.sort_by_key(|c| c.tag.clone());
        libraries.sort_by_key(|c| c.tag.clone());
        models.sort_by_key(|m| m.tag.clone());
        events.sort_by_key(|e| e.tag.clone());
        external_contracts.sort_by_key(|c| c.tag.clone());

        contracts.iter().for_each(|c| add_abi_entries(&mut abis, c.abi.clone()));
        models.iter().for_each(|m| add_abi_entries(&mut abis, m.abi.clone()));
        events.iter().for_each(|e| add_abi_entries(&mut abis, e.abi.clone()));
        libraries.iter().for_each(|l| add_abi_entries(&mut abis, l.abi.clone()));
        external_contracts.iter().for_each(|e| add_abi_entries(&mut abis, e.abi.clone()));

        Self { world, contracts, models, events, libraries, external_contracts, abis }
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

fn resource_diff_to_dojo_external_contract(resource: &ResourceDiff) -> ExternalContract {
    match &resource {
        ResourceDiff::Created(ResourceLocal::ExternalContract(local)) => match local {
            ExternalContractLocal::SozoManaged(l) => ExternalContract {
                class_hash: l.common.class_hash,
                abi: l.common.class.abi.clone(),
                address: l.computed_address,
                constructor_calldata: l.constructor_data.clone(),
                encoded_constructor_calldata: l.encoded_constructor_data.clone(),
                tag: local.tag(),
                contract_name: l.contract_name.clone(),
                entrypoints: l.entrypoints.clone(),
                block_number: l.block_number,
            },
            ExternalContractLocal::SelfManaged(l) => ExternalContract {
                class_hash: Felt::ZERO,
                abi: vec![],
                address: l.contract_address,
                constructor_calldata: vec![],
                encoded_constructor_calldata: vec![],
                tag: local.tag(),
                contract_name: l.name.clone(),
                entrypoints: vec![],
                block_number: Some(l.block_number),
            },
        },
        ResourceDiff::Updated(
            ResourceLocal::ExternalContract(local),
            ResourceRemote::ExternalContract(r),
        )
        | ResourceDiff::Synced(
            ResourceLocal::ExternalContract(local),
            ResourceRemote::ExternalContract(r),
        ) => match local {
            ExternalContractLocal::SozoManaged(l) => ExternalContract {
                class_hash: l.common.class_hash,
                abi: l.common.class.abi.clone(),
                address: if l.is_upgradeable { r.common.address } else { l.computed_address },
                constructor_calldata: l.constructor_data.clone(),
                encoded_constructor_calldata: l.encoded_constructor_data.clone(),
                tag: local.tag(),
                contract_name: l.contract_name.clone(),
                entrypoints: l.entrypoints.clone(),
                block_number: l.block_number,
            },
            ExternalContractLocal::SelfManaged(l) => ExternalContract {
                class_hash: Felt::ZERO,
                abi: vec![],
                address: l.contract_address,
                constructor_calldata: vec![],
                encoded_constructor_calldata: vec![],
                tag: local.tag(),
                contract_name: l.name.clone(),
                entrypoints: vec![],
                block_number: Some(l.block_number),
            },
        },
        _ => unreachable!(),
    }
}

fn resource_diff_to_dojo_library(diff: &WorldDiff, resource: &ResourceDiff) -> DojoLibrary {
    let tag = resource.tag();

    let version = diff
        .profile_config
        .lib_versions
        .as_ref()
        .expect("expected lib_versions")
        .get(&tag)
        .expect("library mush have a version");

    let tag = format!("{}_v{}", tag, version);

    match &resource {
        ResourceDiff::Created(ResourceLocal::Library(l)) => DojoLibrary {
            class_hash: l.common.class_hash,
            abi: l.common.class.abi.clone(),
            tag,
            systems: l.systems.clone(),
            selector: resource.dojo_selector(),
            version: version.clone(),
        },
        ResourceDiff::Updated(ResourceLocal::Library(l), ResourceRemote::Library(_r))
        | ResourceDiff::Synced(ResourceLocal::Library(l), ResourceRemote::Library(_r)) => {
            DojoLibrary {
                class_hash: l.common.class_hash,
                abi: l.common.class.abi.clone(),
                tag,
                systems: l.systems.clone(),
                selector: resource.dojo_selector(),
                version: version.clone(),
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
            abi: l.common.class.abi.clone(),
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
            abi: l.common.class.abi.clone(),
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
            abi: l.common.class.abi.clone(),
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
            abi: l.common.class.abi.clone(),
        },
        _ => unreachable!(),
    }
}

/// Gets the name of the ABI entry.
fn get_abi_name(abi_entry: &AbiEntry) -> String {
    match abi_entry {
        AbiEntry::Function(function) => function.name.clone(),
        AbiEntry::Event(event) => match event {
            AbiEvent::Typed(TypedAbiEvent::Struct(s)) => s.name.clone(),
            AbiEvent::Typed(TypedAbiEvent::Enum(e)) => e.name.clone(),
            AbiEvent::Untyped(UntypedAbiEvent { name, .. }) => name.clone(),
        },
        AbiEntry::Struct(struct_) => struct_.name.clone(),
        AbiEntry::Enum(enum_) => enum_.name.clone(),
        AbiEntry::Constructor(constructor) => constructor.name.clone(),
        AbiEntry::Impl(impl_) => impl_.name.clone(),
        AbiEntry::Interface(interface) => interface.name.clone(),
        AbiEntry::L1Handler(l1_handler) => l1_handler.name.clone(),
    }
}

/// Serializes the ABI entries into a vector sorted alphabetically by name.
/// This ensures compatibility with any tool expecting a Cairo contract ABI.
fn serialize_abis_hashmap<S>(
    value: &HashMap<String, AbiEntry>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut seq = serializer.serialize_seq(Some(value.len()))?;

    // Sort alphabetically by name.
    let mut sorted_entries = value.values().collect::<Vec<_>>();
    sorted_entries.sort_by_key(|e| get_abi_name(e));

    for abi_entry in sorted_entries {
        seq.serialize_element(abi_entry)?;
    }

    seq.end()
}

/// Deserializes the ABI entries from a vector into a hashmap.
fn deserialize_abis_hashmap<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, AbiEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let entries = Vec::<AbiEntry>::deserialize(deserializer)?;

    let mut map = HashMap::new();

    for abi_entry in entries {
        map.insert(get_abi_name(&abi_entry), abi_entry);
    }

    Ok(map)
}

/// Adds the ABI entries to the manifest, deduplicating them by name.
fn add_abi_entries(abis: &mut HashMap<String, AbiEntry>, abi: Vec<AbiEntry>) {
    for abi_entry in abi {
        // We can strip out `impl` type entries, since they are not meaningful for the manifest.
        // Keeping the interface is enough.
        if matches!(abi_entry, AbiEntry::Impl(_)) {
            continue;
        }

        let entry_name = get_abi_name(&abi_entry);
        abis.insert(entry_name, abi_entry);
    }
}
