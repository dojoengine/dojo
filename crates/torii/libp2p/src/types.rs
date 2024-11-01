use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

use crate::typed_data::TypedData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Signature {
    Account(Vec<Felt>),
    Session(Vec<Felt>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message: TypedData,
    pub signature: Signature,
}
