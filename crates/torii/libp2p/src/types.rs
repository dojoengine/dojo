use serde::{Deserialize, Serialize};
use starknet_ff::FieldElement;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ClientMessage {
    pub topic: String,
    pub model: String,
    pub data: Vec<FieldElement>,
}
