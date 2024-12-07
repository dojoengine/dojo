use dojo_types::naming;
use starknet::core::types::contract::{AbiEntry, CompiledClass, SierraClass};
use starknet::core::types::Felt;

use crate::{DojoSelector, ResourceType};

/// A local resource.
#[derive(Debug, Clone)]
pub enum ResourceLocal {
    Namespace(NamespaceLocal),
    Contract(ContractLocal),
    Model(ModelLocal),
    Event(EventLocal),
}

/// Common information about a world's resource.
#[derive(Debug, Clone)]
pub struct CommonLocalInfo {
    /// The name of the contract.
    pub name: String,
    /// The namespace used to register the resource remotely.
    pub namespace: String,
    /// The class of the resource.
    pub class: SierraClass,
    /// The casm class of the resource, optional since it's mostly used for stats.
    pub casm_class: Option<CompiledClass>,
    /// The class hash of the resource.
    pub class_hash: Felt,
    /// The casm class hash of the resource.
    pub casm_class_hash: Felt,
}

#[derive(Debug, Clone)]
pub struct ContractLocal {
    /// Common information about the resource.
    pub common: CommonLocalInfo,
    /// The systems of the contract.
    pub systems: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModelLocal {
    /// Common information about the resource.
    pub common: CommonLocalInfo,
    /// The members of the model.
    pub members: Vec<Member>,
}

#[derive(Debug, Clone)]
pub struct EventLocal {
    /// Common information about the resource.
    pub common: CommonLocalInfo,
    /// The members of the event.
    pub members: Vec<Member>,
}

#[derive(Debug, Clone)]
pub struct NamespaceLocal {
    /// The name of the namespace.
    pub name: String,
}

/// Represents a struct resource like member and event.
#[derive(Debug, Clone)]
pub struct Member {
    /// Name of the member.
    pub name: String,
    /// Type of the member.
    pub ty: String,
    /// Whether the member is a key.
    pub key: bool,
}

impl ResourceLocal {
    /// Returns the name of the resource.
    pub fn name(&self) -> String {
        match self {
            ResourceLocal::Contract(c) => c.common.name.clone(),
            ResourceLocal::Model(m) => m.common.name.clone(),
            ResourceLocal::Event(e) => e.common.name.clone(),
            ResourceLocal::Namespace(n) => n.name.clone(),
        }
    }

    /// Returns the namespace of the resource.
    pub fn namespace(&self) -> String {
        match self {
            ResourceLocal::Namespace(n) => n.name.clone(),
            ResourceLocal::Contract(c) => c.common.namespace.clone(),
            ResourceLocal::Model(m) => m.common.namespace.clone(),
            ResourceLocal::Event(e) => e.common.namespace.clone(),
        }
    }

    /// Returns the class hash of the resource.
    pub fn class_hash(&self) -> Felt {
        match self {
            ResourceLocal::Contract(c) => c.common.class_hash,
            ResourceLocal::Model(m) => m.common.class_hash,
            ResourceLocal::Event(e) => e.common.class_hash,
            _ => Felt::ZERO,
        }
    }

    /// Returns the ABI of the resource.
    pub fn abi(&self) -> Vec<AbiEntry> {
        match self {
            ResourceLocal::Contract(c) => c.common.class.abi.clone(),
            ResourceLocal::Model(m) => m.common.class.abi.clone(),
            ResourceLocal::Event(e) => e.common.class.abi.clone(),
            _ => Vec::new(),
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
        match self {
            ResourceLocal::Namespace(n) => n.name.clone(),
            _ => naming::get_tag(&self.namespace(), &self.name()),
        }
    }

    /// Returns the contract resource.
    ///
    /// This function panics since it must only be used where the developer
    /// can ensure that the resource is a contract.
    pub fn as_contract(&self) -> Option<&ContractLocal> {
        match self {
            ResourceLocal::Contract(c) => Some(c),
            _ => None,
        }
    }

    /// Returns the type of the resource.
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceLocal::Contract(_) => ResourceType::Contract,
            ResourceLocal::Model(_) => ResourceType::Model,
            ResourceLocal::Event(_) => ResourceType::Event,
            ResourceLocal::Namespace(_) => ResourceType::Namespace,
        }
    }

    /// Returns the common information of the resource.
    pub fn common(&self) -> &CommonLocalInfo {
        match self {
            ResourceLocal::Contract(c) => &c.common,
            ResourceLocal::Model(m) => &m.common,
            ResourceLocal::Event(e) => &e.common,
            ResourceLocal::Namespace(_) => panic!("Namespace has no common info."),
        }
    }
}

impl ContractLocal {
    /// Returns the dojo selector of the contract.
    pub fn dojo_selector(&self) -> DojoSelector {
        naming::compute_selector_from_names(&self.common.namespace, &self.common.name)
    }
}
