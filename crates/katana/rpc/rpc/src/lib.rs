//! RPC implementations.

#![allow(clippy::blocks_in_conditions)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::net::SocketAddr;
use std::time::Duration;

use jsonrpsee::server::{AllowHosts, ServerBuilder, ServerHandle};
use jsonrpsee::RpcModule;
use proxy_get_request::DevnetProxyLayer;
use tower::ServiceBuilder;
use tracing::info;

pub mod cors;
pub mod dev;
pub mod health;
pub mod metrics;
pub mod proxy_get_request;
pub mod saya;
pub mod starknet;
pub mod torii;

mod future;
mod logger;
mod transport;
mod utils;

use cors::Cors;
use health::HealthCheck;
use metrics::RpcServerMetrics;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Jsonrpsee(#[from] jsonrpsee::core::Error),

    #[error("RPC server has already been stopped")]
    AlreadyStopped,
}

/// The RPC server handle.
#[derive(Debug, Clone)]
pub struct RpcServerHandle {
    /// The actual address that the server is binded to.
    addr: SocketAddr,
    /// The handle to the spawned [`jsonrpsee::server::Server`].
    handle: ServerHandle,
}

impl RpcServerHandle {
    /// Tell the server to stop without waiting for the server to stop.
    pub fn stop(&self) -> Result<(), Error> {
        self.handle.stop().map_err(|_| Error::AlreadyStopped)
    }

    /// Wait until the server has stopped.
    pub async fn stopped(self) {
        self.handle.stopped().await
    }

    /// Returns the socket address the server is listening on.
    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }
}

#[derive(Debug)]
pub struct RpcServer {
    metrics: bool,
    cors: Option<Cors>,
    health_check: bool,
    module: RpcModule<()>,
    max_connections: u32,
}

impl RpcServer {
    pub fn new() -> Self {
        Self {
            cors: None,
            metrics: false,
            health_check: false,
            max_connections: 100,
            module: RpcModule::new(()),
        }
    }

    /// Collect metrics about the RPC server.
    ///
    /// See top level module of [`crate::metrics`] to see what metrics are collected.
    pub fn metrics(mut self) -> Self {
        self.metrics = true;
        self
    }

    /// Enables health checking endpoint via HTTP `GET /health`
    pub fn health_check(mut self) -> Self {
        self.health_check = true;
        self
    }

    pub fn cors(mut self, cors: Cors) -> Self {
        self.cors = Some(cors);
        self
    }

    pub fn module(mut self, module: RpcModule<()>) -> Self {
        self.module = module;
        self
    }

    pub async fn start(&self, addr: SocketAddr) -> Result<RpcServerHandle, Error> {
        let mut modules = self.module.clone();

        let health_check_proxy = if self.health_check {
            modules.merge(HealthCheck)?;
            Some(HealthCheck::proxy())
        } else {
            None
        };

        let middleware = ServiceBuilder::new()
            .option_layer(self.cors.clone())
            .option_layer(health_check_proxy)
            .layer(DevnetProxyLayer::new()?)
            .timeout(Duration::from_secs(20));

        let builder = ServerBuilder::new()
            .set_middleware(middleware)
            .set_host_filtering(AllowHosts::Any)
            .max_connections(self.max_connections);

        let handle = if self.metrics {
            let logger = RpcServerMetrics::new(&modules);
            let server = builder.set_logger(logger).build(addr).await?;

            let addr = server.local_addr()?;
            let handle = server.start(modules)?;

            RpcServerHandle { addr, handle }
        } else {
            let server = builder.build(addr).await?;

            let addr = server.local_addr()?;
            let handle = server.start(modules)?;

            RpcServerHandle { addr, handle }
        };

        // The socket address that we log out must be from the RPC handle, in the case that the
        // `addr` passed to this method has port number 0. As the 0 port will be resolved to
        // a free port during the call to `ServerBuilder::build(addr)`.

        info!(target: "rpc", addr = %handle.addr, "RPC server started.");

        Ok(handle)
    }
}

impl Default for RpcServer {
    fn default() -> Self {
        Self::new()
    }
}
