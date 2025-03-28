use serde::{Deserialize};
use starknet::core::types::{Felt, TypedData};

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub message: TypedData,
    pub signature: Vec<Felt>,
}
