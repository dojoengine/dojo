//! Remote resources for the world, gathered from events emitted by the world at the given address.
//!
//! A remote resource must be reconstructible form the onchain world without any additional information.

use anyhow::Result;
use std::collections::{HashMap, HashSet};

use dojo_types::naming;
use starknet::core::types::Felt;

mod events_to_remote;
mod permissions;

type DojoSelector = Felt;
type Namespace = String;

/// A remote resource that can be fetched from the world.
#[derive(Debug)]
pub enum RemoteResource {
    Contract(ContractRemote),
    Model(ModelRemote),
    Event(EventRemote),
}

/// The remote world representation.
#[derive(Debug)]
pub struct WorldRemote {
    pub class_hashes: Vec<Felt>,
    pub namespaces: Vec<Namespace>,
    pub contracts: HashMap<Namespace, DojoSelector>,
    pub models: HashMap<Namespace, DojoSelector>,
    pub events: HashMap<Namespace, DojoSelector>,
    pub resources: HashMap<DojoSelector, RemoteResource>,
}

/// Common information about a world's resource.
#[derive(Debug)]
pub struct CommonResourceRemoteInfo {
    /// The class hashes of the resource during its lifecycle,
    /// always at least one if the resource has been registered.
    /// Then for each upgrade, a new class hash is appended to the vector.
    pub class_hashes: Vec<Felt>,
    /// The name of the contract.
    pub name: String,
    /// The address of the contract.
    pub address: Felt,
    /// The contract addresses that have owner permission on the contract.
    pub owners: HashSet<Felt>,
    /// The contract addresses that have writer permission on the contract.
    pub writers: HashSet<Felt>,
}

#[derive(Debug)]
pub struct ContractRemote {
    /// Common information about the resource.
    pub common: CommonResourceRemoteInfo,
    /// Whether the contract has been initialized.
    pub initialized: bool,
}

#[derive(Debug)]
pub struct ModelRemote {
    /// Common information about the resource.
    pub common: CommonResourceRemoteInfo,
}

#[derive(Debug)]
pub struct EventRemote {
    /// Common information about the resource.
    pub common: CommonResourceRemoteInfo,
}

impl Default for WorldRemote {
    fn default() -> Self {
        Self {
            class_hashes: vec![],
            namespaces: vec![],
            contracts: HashMap::new(),
            models: HashMap::new(),
            events: HashMap::new(),
            resources: HashMap::new(),
        }
    }
}

impl CommonResourceRemoteInfo {
    /// Create a new common resource remote info.
    pub fn new(original_class_hash: Felt, name: String, address: Felt) -> Self {
        Self {
            class_hashes: vec![original_class_hash],
            name,
            address,
            owners: HashSet::new(),
            writers: HashSet::new(),
        }
    }

    /// The dojo selector of the resource.
    pub fn dojo_selector(&self, namespace: &Namespace) -> DojoSelector {
        naming::compute_selector_from_names(namespace, &self.name)
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
    pub fn dojo_selector(&self, namespace: &Namespace) -> DojoSelector {
        self.common.dojo_selector(namespace)
    }
}

impl ModelRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self, namespace: &Namespace) -> DojoSelector {
        self.common.dojo_selector(namespace)
    }
}

impl EventRemote {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self, namespace: &Namespace) -> DojoSelector {
        self.common.dojo_selector(namespace)
    }
}

impl RemoteResource {
    /// The dojo selector of the resource.
    pub fn dojo_selector(&self, namespace: &Namespace) -> DojoSelector {
        match self {
            RemoteResource::Contract(contract) => contract.dojo_selector(namespace),
            RemoteResource::Model(model) => model.dojo_selector(namespace),
            RemoteResource::Event(event) => event.dojo_selector(namespace),
        }
    }

    /// Push a new class hash to the resource meaning it has been upgraded.
    pub fn push_class_hash(&mut self, class_hash: Felt) {
        match self {
            RemoteResource::Contract(contract) => contract.common.push_class_hash(class_hash),
            RemoteResource::Model(model) => model.common.push_class_hash(class_hash),
            RemoteResource::Event(event) => event.common.push_class_hash(class_hash),
        }
    }

    /// Get the contract remote if the resource is a contract, otherwise return an error.
    pub fn as_contract_mut(&mut self) -> Result<&mut ContractRemote> {
        match self {
            RemoteResource::Contract(contract) => Ok(contract),
            _ => anyhow::bail!("Resource is expected to be a contract: {:?}.", self),
        }
    }
}
