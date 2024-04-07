use serde::{Deserialize, Serialize};
use starknet_crypto::FieldElement;

use crate::typed_data::TypedData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message: TypedData,
    pub signature_r: FieldElement,
    pub signature_s: FieldElement,
}
