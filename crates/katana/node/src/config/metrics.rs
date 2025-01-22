use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// Metrics server default address.
pub const DEFAULT_METRICS_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
/// Metrics server default port.
pub const DEFAULT_METRICS_PORT: u16 = 9100;

/// Node metrics configurations.
#[derive(Debug, Copy, Clone)]
pub struct MetricsConfig {
    /// The address to bind the metrics server to.
    pub addr: IpAddr,
    /// The port to bind the metrics server to.
    pub port: u16,
}

impl MetricsConfig {
    /// Returns the [`SocketAddr`] for the metrics server.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.addr, self.port)
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        MetricsConfig { addr: DEFAULT_METRICS_ADDR, port: DEFAULT_METRICS_PORT }
    }
}
