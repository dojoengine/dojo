use std::collections::HashSet;

use anyhow::Result;
use dojo_types::naming;
use starknet::core::types::Felt;

use crate::{ContractAddress, DojoSelector, ResourceType};

/// A remote resource that can be fetched from the world.
#[derive(Debug, Clone)]
pub enum ResourceRemote {
    Namespace(NamespaceRemote),
    Contract(ContractRemote),
    ExternalContract(ExternalContractRemote),
    Model(ModelRemote),
    Event(EventRemote),
    Library(LibraryRemote),
    // TODO: add starknet contract remote. Sozo needs a way to keep track of the address of this
    // contract once deployed.
}

/// Common information about a world's resource.
#[derive(Debug, Clone)]
pub struct CommonRemoteInfo {
    /// The class hashes of the resource during its lifecycle,
    /// always at least one if the resource has been registered.
    /// Then for each upgrade, a new class hash is appended to the vector.
    pub class_hashes: Vec<Felt>,
    /// The name of the contract.
    pub name: String,
    /// The namespace used to register the resource remotely.
    pub namespace: String,
    /// The address of the resource.
    pub address: ContractAddress,
    /// The hash of the stored metadata associated to the resource if any.
    pub metadata_hash: Felt,
    /// The contract addresses that have owner permission on the resource.
    pub owners: HashSet<ContractAddress>,
    /// The contract addresses that have writer permission on the resource.
    pub writers: HashSet<ContractAddress>,
}

#[derive(Debug, Clone)]
pub struct ContractRemote {
    /// Common information about the resource.
    pub common: CommonRemoteInfo,
    /// Whether the contract has been initialized.
    pub is_initialized: bool,
}

#[derive(Debug, Clone)]
pub struct ExternalContractRemote {
    /// Common information about the resource.
    pub common: CommonRemoteInfo,
}

#[derive(Debug, Clone)]
pub struct LibraryRemote {
    /// Common information about the resource.
    pub common: CommonRemoteInfo,
    /// Version
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct ModelRemote {
    /// Common information about the resource.
    pub common: CommonRemoteInfo,
}

#[derive(Debug, Clone)]
pub struct EventRemote {
    /// Common information about the resource.
    pub common: CommonRemoteInfo,
}

#[derive(Debug, Clone)]
pub struct NamespaceRemote {
    pub name: String,
    /// The contract addresses that have owner permission on the contract.
    pub owners: HashSet<ContractAddress>,
    /// The contract addresses that have writer permission on the contract.
    pub writers: HashSet<ContractAddress>,
}

impl NamespaceRemote {
    /// Create a new namespace remote.
    pub fn new(name: String) -> Self {
        Self { name, owners: HashSet::new(), writers: HashSet::new() }
    }
}

impl CommonRemoteInfo {
    /// Create a new common resource remote info.
    pub fn new(original_class_hash: Felt, namespace: &str, name: &str, address: Felt) -> Self {
        Self {
            class_hashes: vec![original_class_hash],
            name: name.to_string(),
            namespace: namespace.to_string(),
            address,
            metadata_hash: Felt::ZERO,
            owners: HashSet::new(),
            writers: HashSet::new(),
        }
    }

    /// The dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        naming::compute_selector_from_names(&self.namespace, &self.name)
    }

    /// The class hash of the resource after its latest upgrade.
    pub fn current_class_hash(&self) -> Felt {
        *self.class_hashes.last().expect("Remote resources must have at least one class hash.")
    }

    /// The class hash of the resource when it was first registered.
    pub fn original_class_hash(&self) -> Felt {
        *self.class_hashes.first().expect("Remote resources must have at least one class hash.")
    }

    /// Push a new class hash to the resource meaning it has been upgraded.
    pub fn push_class_hash(&mut self, class_hash: Felt) {
        self.class_hashes.push(class_hash);
    }
}

impl ContractRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        self.common.dojo_selector()
    }
}

impl ExternalContractRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        self.common.dojo_selector()
    }
}

impl LibraryRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        self.common.dojo_selector()
    }
}

impl ModelRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        self.common.dojo_selector()
    }
}

impl EventRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        self.common.dojo_selector()
    }
}

impl ResourceRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        match self {
            // The namespace doesn't apply to have the dojo selector of a namespace resource.
            ResourceRemote::Namespace(ns) => naming::compute_bytearray_hash(&ns.name),
            ResourceRemote::Contract(contract) => contract.dojo_selector(),
            ResourceRemote::ExternalContract(contract) => contract.dojo_selector(),
            ResourceRemote::Model(model) => model.dojo_selector(),
            ResourceRemote::Event(event) => event.dojo_selector(),
            ResourceRemote::Library(library) => library.dojo_selector(),
        }
    }
    /// The name of the resource.
    pub fn name(&self) -> String {
        match self {
            ResourceRemote::Contract(c) => c.common.name.clone(),
            ResourceRemote::ExternalContract(c) => c.common.name.clone(),
            ResourceRemote::Model(m) => m.common.name.clone(),
            ResourceRemote::Event(e) => e.common.name.clone(),
            ResourceRemote::Namespace(ns) => ns.name.clone(),
            ResourceRemote::Library(l) => l.common.name.clone(),
        }
    }

    /// The namespace of the resource.
    pub fn namespace(&self) -> String {
        match self {
            ResourceRemote::Contract(c) => c.common.namespace.clone(),
            ResourceRemote::ExternalContract(c) => c.common.namespace.clone(),
            ResourceRemote::Model(m) => m.common.namespace.clone(),
            ResourceRemote::Event(e) => e.common.namespace.clone(),
            ResourceRemote::Namespace(ns) => ns.name.clone(),
            ResourceRemote::Library(l) => l.common.namespace.clone(),
        }
    }

    /// Returns the tag of the resource.
    pub fn tag(&self) -> String {
        naming::get_tag(&self.namespace(), &self.name())
    }

    /// The address of the resource.
    pub fn address(&self) -> Felt {
        match self {
            ResourceRemote::Contract(c) => c.common.address,
            ResourceRemote::ExternalContract(c) => c.common.address,
            ResourceRemote::Model(m) => m.common.address,
            ResourceRemote::Event(e) => e.common.address,
            ResourceRemote::Namespace(_) => Felt::ZERO,
            ResourceRemote::Library(_) => Felt::ZERO,
        }
    }

    /// Set the hash of the stored metadata associated to the resource.
    pub fn set_metadata_hash(&mut self, hash: Felt) {
        match self {
            ResourceRemote::Contract(c) => c.common.metadata_hash = hash,
            ResourceRemote::ExternalContract(_) => {}
            ResourceRemote::Model(m) => m.common.metadata_hash = hash,
            ResourceRemote::Event(e) => e.common.metadata_hash = hash,
            ResourceRemote::Namespace(_) => {}
            ResourceRemote::Library(l) => l.common.metadata_hash = hash,
        }
    }

    /// The hash of the stored metadata associated to the resource.
    pub fn metadata_hash(&self) -> Felt {
        match self {
            ResourceRemote::Contract(c) => c.common.metadata_hash,
            ResourceRemote::ExternalContract(_) => Felt::ZERO,
            ResourceRemote::Model(m) => m.common.metadata_hash,
            ResourceRemote::Event(e) => e.common.metadata_hash,
            ResourceRemote::Namespace(_) => Felt::ZERO,
            ResourceRemote::Library(l) => l.common.metadata_hash,
        }
    }

    /// Push a new class hash to the resource meaning it has been upgraded.
    pub fn push_class_hash(&mut self, class_hash: Felt) {
        match self {
            ResourceRemote::Namespace(_) => {}
            ResourceRemote::Contract(contract) => contract.common.push_class_hash(class_hash),
            ResourceRemote::ExternalContract(contract) => {
                contract.common.push_class_hash(class_hash)
            }
            ResourceRemote::Model(model) => model.common.push_class_hash(class_hash),
            ResourceRemote::Event(event) => event.common.push_class_hash(class_hash),
            ResourceRemote::Library(library) => library.common.push_class_hash(class_hash),
        }
    }

    /// The class hash of the resource after its latest upgrade.
    pub fn current_class_hash(&self) -> Felt {
        match self {
            ResourceRemote::Contract(contract) => contract.common.current_class_hash(),
            ResourceRemote::ExternalContract(contract) => contract.common.current_class_hash(),
            ResourceRemote::Model(model) => model.common.current_class_hash(),
            ResourceRemote::Event(event) => event.common.current_class_hash(),
            ResourceRemote::Namespace(_) => Felt::ZERO,
            ResourceRemote::Library(library) => library.common.current_class_hash(),
        }
    }

    /// Get the writers of the resource and it's dojo selector.
    pub fn get_writers(&self) -> (DojoSelector, HashSet<Felt>) {
        match self {
            ResourceRemote::Contract(contract) => {
                (self.dojo_selector(), contract.common.writers.clone())
            }
            ResourceRemote::ExternalContract(contract) => {
                (self.dojo_selector(), contract.common.writers.clone())
            }
            ResourceRemote::Model(model) => (self.dojo_selector(), model.common.writers.clone()),
            ResourceRemote::Event(event) => (self.dojo_selector(), event.common.writers.clone()),
            ResourceRemote::Namespace(ns) => (self.dojo_selector(), ns.writers.clone()),
            ResourceRemote::Library(library) => {
                (self.dojo_selector(), library.common.writers.clone())
            }
        }
    }

    /// Get the owners of the resource and it's dojo selector.
    pub fn get_owners(&self) -> (DojoSelector, HashSet<Felt>) {
        match self {
            ResourceRemote::Contract(contract) => {
                (self.dojo_selector(), contract.common.owners.clone())
            }
            ResourceRemote::ExternalContract(contract) => {
                (self.dojo_selector(), contract.common.owners.clone())
            }
            ResourceRemote::Model(model) => (self.dojo_selector(), model.common.owners.clone()),
            ResourceRemote::Event(event) => (self.dojo_selector(), event.common.owners.clone()),
            ResourceRemote::Namespace(ns) => (self.dojo_selector(), ns.owners.clone()),
            ResourceRemote::Library(library) => {
                (self.dojo_selector(), library.common.owners.clone())
            }
        }
    }

    /// Returns the type of the resource.
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceRemote::Contract(_) => ResourceType::Contract,
            ResourceRemote::ExternalContract(_) => ResourceType::ExternalContract,
            ResourceRemote::Model(_) => ResourceType::Model,
            ResourceRemote::Event(_) => ResourceType::Event,
            ResourceRemote::Namespace(_) => ResourceType::Namespace,
            ResourceRemote::Library(_) => ResourceType::Library,
        }
    }

    /// Get the contract remote if the resource is a contract, otherwise return an error.
    pub fn as_contract_mut(&mut self) -> Result<&mut ContractRemote> {
        match self {
            ResourceRemote::Contract(contract) => Ok(contract),
            _ => anyhow::bail!("Resource is expected to be a contract: {:?}.", self),
        }
    }

    /// Get the contract remote if the resource is a contract, otherwise panic.
    pub fn as_contract_or_panic(&self) -> &ContractRemote {
        match self {
            ResourceRemote::Contract(contract) => contract,
            _ => panic!("Resource is expected to be a contract: {:?}.", self),
        }
    }

    /// Get the external contract remote if the resource is an external contract, otherwise panic.
    pub fn as_external_contract_or_panic(&self) -> &ExternalContractRemote {
        match self {
            ResourceRemote::ExternalContract(contract) => contract,
            _ => panic!("Resource is expected to be an external contract: {:?}.", self),
        }
    }

    /// Get the library remote if the resource is a library, otherwise panic.
    pub fn as_library_or_panic(&self) -> &LibraryRemote {
        match self {
            ResourceRemote::Library(library) => library,
            _ => panic!("Resource is expected to be a library: {:?}.", self),
        }
    }

    /// Get the model remote if the resource is a model, otherwise panic.
    pub fn as_model_or_panic(&self) -> &ModelRemote {
        match self {
            ResourceRemote::Model(model) => model,
            _ => panic!("Resource is expected to be a model: {:?}.", self),
        }
    }

    /// Get the event remote if the resource is an event, otherwise panic.
    pub fn as_event_or_panic(&self) -> &EventRemote {
        match self {
            ResourceRemote::Event(event) => event,
            _ => panic!("Resource is expected to be an event: {:?}.", self),
        }
    }

    /// Get the namespace remote if the resource is a namespace, otherwise panic.
    pub fn as_namespace_or_panic(&self) -> &NamespaceRemote {
        match self {
            ResourceRemote::Namespace(namespace) => namespace,
            _ => panic!("Resource is expected to be a namespace: {:?}.", self),
        }
    }
}
