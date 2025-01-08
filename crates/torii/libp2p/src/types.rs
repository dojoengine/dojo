use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

use torii_typed_data::TypedData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message: TypedData,
    pub signature: Vec<Felt>,
}
