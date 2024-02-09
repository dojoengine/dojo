use dojo_types::schema::Ty;
use serde::{Deserialize, Serialize};
use starknet_ff::FieldElement;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ClientMessage {
    pub topic: String,
    pub data: Ty,
}
