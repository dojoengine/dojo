use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

use crate::typed_data::TypedData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message: TypedData,
    pub signature_r: Felt,
    pub signature_s: Felt,
}
