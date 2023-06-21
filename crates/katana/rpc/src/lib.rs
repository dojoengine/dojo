use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use config::ServerConfig;
use hyper::Method;
use jsonrpsee::server::{AllowHosts, ServerBuilder, ServerHandle};
use katana::api::KatanaApiServer;
use katana::KatanaRpc;
use katana_core::sequencer::Sequencer;
use tower_http::cors::{Any, CorsLayer};

pub mod config;
mod katana;
mod starknet;
mod utils;

use self::starknet::api::StarknetApiServer;
use self::starknet::rpc::StarknetRpc;

#[derive(Debug, Clone)]
pub struct KatanaNodeRpc<S> {
    pub config: ServerConfig,
    pub sequencer: Arc<S>,
}

impl<S> KatanaNodeRpc<S>
where
    S: Sequencer + Send + Sync + 'static,
{
    pub fn new(sequencer: Arc<S>, config: ServerConfig) -> Self {
        Self { config, sequencer }
    }

    pub async fn run(self) -> Result<(SocketAddr, ServerHandle)> {
        let mut methods = KatanaRpc::new(self.sequencer.clone()).into_rpc();
        methods.merge(StarknetRpc::new(self.sequencer.clone()).into_rpc())?;

        let cors = CorsLayer::new()
            // Allow `POST` when accessing the resource
            .allow_methods([Method::POST])
            // Allow requests from any origin
            .allow_origin(Any)
            .allow_headers([hyper::header::CONTENT_TYPE]);
        let middleware = tower::ServiceBuilder::new().layer(cors);

        let server = ServerBuilder::new()
            .set_logger(KatanaNodeRpcLogger)
            .set_host_filtering(AllowHosts::Any)
            .set_middleware(middleware)
            .build(self.config.addr())
            .await?;

        let addr = server.local_addr()?;
        let handle = server.start(methods)?;

        Ok((addr, handle))
    }
}

use std::time::Instant;

use jsonrpsee::server::logger::{Logger, MethodKind, TransportProtocol};
use jsonrpsee::tracing::debug;
use jsonrpsee::types::Params;

#[derive(Debug, Clone)]
pub struct KatanaNodeRpcLogger;

impl Logger for KatanaNodeRpcLogger {
    type Instant = std::time::Instant;

    fn on_connect(
        &self,
        _remote_addr: std::net::SocketAddr,
        _request: &jsonrpsee::server::logger::HttpRequest,
        _t: TransportProtocol,
    ) {
    }

    fn on_request(&self, _transport: TransportProtocol) -> Self::Instant {
        Instant::now()
    }

    fn on_call(
        &self,
        method_name: &str,
        _params: Params<'_>,
        _kind: MethodKind,
        _transport: TransportProtocol,
    ) {
        debug!(method = ?method_name);
    }

    fn on_result(
        &self,
        _method_name: &str,
        _success: bool,
        _started_at: Self::Instant,
        _transport: TransportProtocol,
    ) {
    }

    fn on_response(
        &self,
        _result: &str,
        _started_at: Self::Instant,
        _transport: TransportProtocol,
    ) {
    }
    fn on_disconnect(&self, _remote_addr: std::net::SocketAddr, _transport: TransportProtocol) {}
}
