use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

use crate::typed_data::TypedData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedDataHash {
    pub type_hash: Felt,
    pub typed_data_hash: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Signature {
    Webauthn(Vec<Felt>),
    Starknet((Felt, Felt)),
    Session((Vec<TypedDataHash>, Vec<Felt>)),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message: TypedData,
    pub signature: Signature,
}
