use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ClientMessage {
    pub topic: String,
    pub data: Vec<u8>,
}
