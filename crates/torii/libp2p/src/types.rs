use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use torii_sqlite::types::{ContractCursor, Entity, Event, EventMessage, Model};
use torii_typed_data::TypedData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message: TypedData,
    pub signature: Vec<Felt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Update {
    // Latest contract head
    Head(ContractCursor),
    // Registered model
    Model(Model),
    // Updated entity state
    Entity(Entity),
    // Indexed event message
    EventMessage(EventMessage),
    // Indexed raw event
    Event(Event),
    // TODO: Add more types of updates here.
}
