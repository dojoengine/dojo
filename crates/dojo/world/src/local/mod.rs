//! Local resources for the world, gathered from the Scarb generated artifacts.

use std::collections::HashMap;

use starknet::core::types::contract::SierraClass;

mod artifact_to_local;

#[derive(Debug)]
pub enum LocalResource {
    Contract(ContractLocal),
    Model(ModelLocal),
    Event(EventLocal),
    Starknet(StarknetLocal),
}

#[derive(Debug)]
pub struct WorldLocal {
    pub class: Option<SierraClass>,
    pub contracts: HashMap<String, ContractLocal>,
    pub models: HashMap<String, ModelLocal>,
    pub events: HashMap<String, EventLocal>,
    pub starknet_contracts: HashMap<String, StarknetLocal>,
}

#[derive(Debug)]
pub struct ContractLocal {
    pub class: SierraClass,
    // TODO: add systems for better debugging/more info for users.
}

#[derive(Debug)]
pub struct ModelLocal {
    pub class: SierraClass,
}

#[derive(Debug)]
pub struct EventLocal {
    pub class: SierraClass,
}

#[derive(Debug)]
pub struct StarknetLocal {
    pub class: SierraClass,
}

impl Default for WorldLocal {
    fn default() -> Self {
        Self {
            class: None,
            contracts: HashMap::new(),
            models: HashMap::new(),
            events: HashMap::new(),
            starknet_contracts: HashMap::new(),
        }
    }
}
