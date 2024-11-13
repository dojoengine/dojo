use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// The default maximum number of concurrent RPC connections.
pub const DEFAULT_RPC_MAX_CONNECTIONS: u32 = 100;
pub const DEFAULT_RPC_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
pub const DEFAULT_RPC_PORT: u16 = 5050;
pub const DEFAULT_RPC_PAGE_SIZE: u64 = 100;
/// List of APIs supported by Katana.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, strum_macros::EnumString, strum_macros::Display,
)]
pub enum ApiKind {
    Starknet,
    Torii,
    Dev,
    Saya,
}

/// Configuration for the RPC server.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub addr: IpAddr,
    pub port: u16,
    pub max_connections: u32,
    pub apis: HashSet<ApiKind>,
    pub max_event_page_size: Option<u64>,
    pub cors_origins: Option<Vec<String>>,
}

impl RpcConfig {
    /// Returns the [`SocketAddr`] for the RPC server.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.addr, self.port)
    }
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            cors_origins: None,
            addr: DEFAULT_RPC_ADDR,
            port: DEFAULT_RPC_PORT,
            max_connections: DEFAULT_RPC_MAX_CONNECTIONS,
            apis: HashSet::from([ApiKind::Starknet]),
            max_event_page_size: Some(DEFAULT_RPC_PAGE_SIZE),
        }
    }
}
