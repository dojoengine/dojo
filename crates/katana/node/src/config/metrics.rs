use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct MetricsConfig {
    pub addr: SocketAddr,
}
