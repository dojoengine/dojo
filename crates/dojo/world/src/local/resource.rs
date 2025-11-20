use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use dojo_types::naming;
use starknet::core::types::contract::{AbiEntry, SierraClass};
use starknet::core::types::Felt;

use crate::{DojoSelector, ResourceType};

/// A local resource.
#[derive(Debug, Clone)]
pub enum ResourceLocal {
    Namespace(NamespaceLocal),
    Contract(ContractLocal),
    ExternalContract(ExternalContractLocal),
    Model(ModelLocal),
    Event(EventLocal),
    Library(LibraryLocal),
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
    pub casm_class: Option<CasmContractClass>,
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
#[allow(clippy::large_enum_variant)]
pub enum ExternalContractLocal {
    SelfManaged(SelfManagedExternalContractLocal),
    SozoManaged(SozoManagedExternalContractLocal),
}

#[derive(Debug, Clone)]
pub struct SelfManagedExternalContractLocal {
    /// The name of the contract.
    pub name: String,
    /// The namespace used to register the resource remotely.
    pub namespace: String,
    // the contract address
    pub contract_address: Felt,
    // the block number from where to start indexing
    pub block_number: u64,
}

#[derive(Debug, Clone)]
pub struct SozoManagedExternalContractLocal {
    /// Common information about the resource.
    pub common: CommonLocalInfo,
    // The Cairo contract name (common.name is the instance name)
    pub contract_name: String,
    // Salt used to deploy the contract
    pub salt: Felt,
    // Human-readeable constructor data
    pub constructor_data: Vec<String>,
    // encoded data to pass to the constructor while deploying the contract
    pub encoded_constructor_data: Vec<Felt>,
    // the computed contract address
    pub computed_address: Felt,
    // list of exported entry points of the contract
    pub entrypoints: Vec<String>,
    // indicates if the contract is upgradeable or if it has to be
    // deployed at another address in case of upgrade.
    pub is_upgradeable: bool,
    // the block number from where to start indexing
    pub block_number: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LibraryLocal {
    /// Common information about the resource.
    pub common: CommonLocalInfo,
    /// The systems of the library.
    pub systems: Vec<String>,
    /// The version of the library
    pub version: String,
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
            ResourceLocal::ExternalContract(c) => match c {
                ExternalContractLocal::SozoManaged(c) => c.common.name.clone(),
                ExternalContractLocal::SelfManaged(c) => c.name.clone(),
            },
            ResourceLocal::Model(m) => m.common.name.clone(),
            ResourceLocal::Event(e) => e.common.name.clone(),
            ResourceLocal::Namespace(n) => n.name.clone(),
            ResourceLocal::Library(l) => l.common.name.clone(),
        }
    }

    /// Returns the namespace of the resource.
    pub fn namespace(&self) -> String {
        match self {
            ResourceLocal::Namespace(n) => n.name.clone(),
            ResourceLocal::Contract(c) => c.common.namespace.clone(),
            ResourceLocal::ExternalContract(c) => match c {
                ExternalContractLocal::SozoManaged(c) => c.common.namespace.clone(),
                ExternalContractLocal::SelfManaged(c) => c.namespace.clone(),
            },
            ResourceLocal::Model(m) => m.common.namespace.clone(),
            ResourceLocal::Event(e) => e.common.namespace.clone(),
            ResourceLocal::Library(l) => l.common.namespace.clone(),
        }
    }

    /// Returns the class hash of the resource.
    pub fn class_hash(&self) -> Felt {
        match self {
            ResourceLocal::Contract(c) => c.common.class_hash,
            ResourceLocal::ExternalContract(c) => match c {
                ExternalContractLocal::SozoManaged(c) => c.common.class_hash,
                ExternalContractLocal::SelfManaged(_) => Felt::ZERO,
            },
            ResourceLocal::Model(m) => m.common.class_hash,
            ResourceLocal::Event(e) => e.common.class_hash,
            ResourceLocal::Library(l) => l.common.class_hash,
            _ => Felt::ZERO,
        }
    }

    /// Returns the ABI of the resource.
    pub fn abi(&self) -> Vec<AbiEntry> {
        match self {
            ResourceLocal::Contract(c) => c.common.class.abi.clone(),
            ResourceLocal::ExternalContract(c) => match c {
                ExternalContractLocal::SozoManaged(c) => c.common.class.abi.clone(),
                ExternalContractLocal::SelfManaged(_) => Vec::new(),
            },
            ResourceLocal::Model(m) => m.common.class.abi.clone(),
            ResourceLocal::Event(e) => e.common.class.abi.clone(),
            ResourceLocal::Library(l) => l.common.class.abi.clone(),
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

    /// Returns the external contract resource.
    ///
    /// This function panics since it must only be used where the developer
    /// can ensure that the resource is an external contract.
    pub fn as_external_contract(&self) -> Option<&ExternalContractLocal> {
        match self {
            ResourceLocal::ExternalContract(c) => Some(c),
            _ => None,
        }
    }

    /// Returns the type of the resource.
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceLocal::Contract(_) => ResourceType::Contract,
            ResourceLocal::ExternalContract(_) => ResourceType::ExternalContract,
            ResourceLocal::Model(_) => ResourceType::Model,
            ResourceLocal::Event(_) => ResourceType::Event,
            ResourceLocal::Namespace(_) => ResourceType::Namespace,
            ResourceLocal::Library(_) => ResourceType::Library,
        }
    }

    /// Returns the common information of the resource.
    pub fn common(&self) -> &CommonLocalInfo {
        match self {
            ResourceLocal::Contract(c) => &c.common,
            ResourceLocal::ExternalContract(c) => match c {
                ExternalContractLocal::SozoManaged(c) => &c.common,
                ExternalContractLocal::SelfManaged(_) => {
                    panic!("Self-managed external contract has no common info.")
                }
            },
            ResourceLocal::Model(m) => &m.common,
            ResourceLocal::Event(e) => &e.common,
            ResourceLocal::Namespace(_) => panic!("Namespace has no common info."),
            ResourceLocal::Library(l) => &l.common,
        }
    }
}

impl ContractLocal {
    /// Returns the dojo selector of the contract.
    pub fn dojo_selector(&self) -> DojoSelector {
        naming::compute_selector_from_names(&self.common.namespace, &self.common.name)
    }
}

impl ExternalContractLocal {
    /// Returns the tag of the resource.
    pub fn tag(&self) -> String {
        naming::get_tag(&self.namespace(), &self.name())
    }

    /// Returns the name of the resource.
    pub fn name(&self) -> String {
        match self {
            Self::SozoManaged(c) => c.common.name.clone(),
            Self::SelfManaged(c) => c.name.clone(),
        }
    }

    /// Returns the namespace of the resource.
    pub fn namespace(&self) -> String {
        match self {
            Self::SozoManaged(c) => c.common.namespace.clone(),
            Self::SelfManaged(c) => c.namespace.clone(),
        }
    }
}
