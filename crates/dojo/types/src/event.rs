use starknet::core::types::Felt;

/// The event emitted when a World is spawned.
#[derive(Clone, Debug)]
pub struct WorldSpawned {
    pub address: Felt,
    pub caller: Felt,
}

/// The event emitted when a model is registered to a World.
#[derive(Clone, Debug)]
pub struct ModelRegistered {
    pub name: String,
    pub class_hash: Felt,
}

/// The event emitted when a model value of an entity is set.
#[derive(Clone, Debug)]
pub struct StoreSetRecord {
    pub table_id: Felt,
    pub keys: Vec<Felt>,
    pub offset: u8,
    pub value: Vec<Felt>,
}

/// The event emitted when a model is deleted from an entity.
#[derive(Clone, Debug)]
pub struct StoreDelRecord {
    pub table_id: Felt,
    pub keys: Vec<Felt>,
}
