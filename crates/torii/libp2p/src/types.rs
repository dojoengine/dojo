use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientMessage {
    pub topic: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerMessage {
    pub peer_id: String,
    pub data: Vec<u8>,
}