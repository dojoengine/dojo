//! Remote resources for the world, gathered from events emitted by the world at the given address.

use std::collections::HashMap;

use starknet::core::types::Felt;

mod events_to_remote;

type Namespace = String;

#[derive(Debug)]
pub enum RemoteResource {
    Contract(ContractRemote),
    Model(ModelRemote),
    Event(EventRemote),
    Starknet(StarknetRemote),
}

#[derive(Debug)]
pub struct WorldRemote {
    pub original_class_hash: Felt,
    pub current_class_hash: Felt,
    pub namespaces: Vec<String>,
    pub contracts: HashMap<Namespace, HashMap<String, ContractRemote>>,
    pub models: HashMap<Namespace, HashMap<String, ModelRemote>>,
    pub events: HashMap<Namespace, HashMap<String, EventRemote>>,
    pub starknet_contracts: HashMap<Namespace, StarknetRemote>,
}

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
    pub owners: Vec<Felt>,
    /// The contract addresses that have writer permission on the contract.
    pub writers: Vec<Felt>,
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

#[derive(Debug)]
pub struct StarknetRemote {
    /// The name of the contract.
    pub name: String,
    /// The address of the contract.
    pub address: Felt,
}

impl Default for WorldRemote {
    fn default() -> Self {
        Self {
            original_class_hash: Felt::ZERO,
            current_class_hash: Felt::ZERO,
            namespaces: vec![],
            contracts: HashMap::new(),
            models: HashMap::new(),
            events: HashMap::new(),
            starknet_contracts: HashMap::new(),
        }
    }
}

impl CommonResourceRemoteInfo {
    /// The class hash of the resource after its latest upgrade.
    pub fn current_class_hash(&self) -> Felt {
        *self.class_hashes.last().expect("Remote resources must have at least one class hash.")
    }

    /// The class hash of the resource when it was first registered.
    pub fn original_class_hash(&self) -> Felt {
        *self.class_hashes.first().expect("Remote resources must have at least one class hash.")
    }
}
