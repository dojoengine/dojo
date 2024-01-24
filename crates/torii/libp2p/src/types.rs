use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ClientMessage {
    pub topic: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ServerMessage {
    pub peer_id: Vec<u8>,
    pub data: Vec<u8>,
}
