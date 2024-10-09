use std::net::SocketAddr;

/// Node metrics configurations.
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// The address to bind the metrics server to.
    pub addr: SocketAddr,
}
