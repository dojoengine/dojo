//! RPC implementations.

#![allow(clippy::blocks_in_conditions)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::net::SocketAddr;
use std::time::Duration;

use jsonrpsee::server::{AllowHosts, ServerBuilder, ServerHandle};
use jsonrpsee::RpcModule;
use tower::ServiceBuilder;
use tracing::info;

pub mod cors;
pub mod dev;
pub mod health;
pub mod metrics;
pub mod saya;
pub mod starknet;
pub mod torii;
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

#[derive(Debug)]
pub struct RpcServerHandle {
    pub addr: SocketAddr,
    pub handle: ServerHandle,
}

impl RpcServerHandle {
    pub fn stop(&self) -> Result<(), Error> {
        self.handle.stop().map_err(|_| Error::AlreadyStopped)
    }

    /// Wait until the server has stopped.
    pub async fn stopped(self) {
        self.handle.stopped().await
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

        info!(target: "rpc", %addr, "RPC server started.");

        Ok(handle)
    }
}

impl Default for RpcServer {
    fn default() -> Self {
        Self::new()
    }
}
