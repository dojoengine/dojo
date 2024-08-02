use std::net::SocketAddr;

use katana_rpc_api::ApiKind;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub max_connections: u32,
    pub allowed_origins: Option<Vec<String>>,
    pub apis: Vec<ApiKind>,
    pub metrics: Option<SocketAddr>,
}

impl ServerConfig {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
