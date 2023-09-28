use starknet::core::types::FieldElement;

/// The event emitted when a World is spawned.
#[derive(Clone, Debug)]
pub struct WorldSpawned {
    pub address: FieldElement,
    pub caller: FieldElement,
}

/// The event emitted when a model is registered to a World.
#[derive(Clone, Debug)]
pub struct ModelRegistered {
    pub name: String,
    pub class_hash: FieldElement,
}

/// The event emmitted when a model value of an entity is set.
#[derive(Clone, Debug)]
pub struct StoreSetRecord {
    pub table_id: FieldElement,
    pub keys: Vec<FieldElement>,
    pub offset: u8,
    pub value: Vec<FieldElement>,
}

/// The event emmitted when a model is deleted from an entity.
#[derive(Clone, Debug)]
pub struct StoreDelRecord {
    pub table_id: FieldElement,
    pub keys: Vec<FieldElement>,
}
