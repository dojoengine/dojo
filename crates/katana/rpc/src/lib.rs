mod api;
pub mod config;
mod katana;
mod starknet;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use anyhow::Result;
use config::ServerConfig;
use hyper::Method;
use jsonrpsee::server::logger::{Logger, MethodKind, TransportProtocol};
use jsonrpsee::server::middleware::proxy_get_request::ProxyGetRequestLayer;
use jsonrpsee::server::{AllowHosts, ServerBuilder, ServerHandle};
use jsonrpsee::tracing::debug;
use jsonrpsee::types::Params;
use jsonrpsee::RpcModule;
use tower_http::cors::{Any, CorsLayer};

use crate::api::katana::KatanaApiServer;
use crate::api::starknet::StarknetApiServer;
pub use crate::katana::KatanaApi;
pub use crate::starknet::StarknetApi;

pub async fn spawn(
    katana_api: KatanaApi,
    starknet_api: StarknetApi,
    config: ServerConfig,
) -> Result<NodeHandle> {
    let mut methods = RpcModule::new(());
    methods.merge(starknet_api.into_rpc())?;
    methods.merge(katana_api.into_rpc())?;
    methods.register_method("health", |_, _| Ok(serde_json::json!({ "health": true })))?;

    let cors = CorsLayer::new()
            // Allow `POST` when accessing the resource
            .allow_methods([Method::POST, Method::GET])
            // Allow requests from any origin
            .allow_origin(Any)
            .allow_headers([hyper::header::CONTENT_TYPE]);

    let middleware = tower::ServiceBuilder::new()
        .layer(cors)
        .layer(ProxyGetRequestLayer::new("/", "health")?)
        .timeout(Duration::from_secs(2));

    let server = ServerBuilder::new()
        .set_logger(RpcLogger)
        .set_host_filtering(AllowHosts::Any)
        .set_middleware(middleware)
        .build(config.addr())
        .await?;

    let addr = server.local_addr()?;
    let handle = server.start(methods)?;

    Ok(NodeHandle { config, handle, addr })
}

#[derive(Debug, Clone)]
pub struct NodeHandle {
    pub addr: SocketAddr,
    pub config: ServerConfig,
    pub handle: ServerHandle,
}

#[derive(Debug, Clone)]
pub struct RpcLogger;

impl Logger for RpcLogger {
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
        debug!(target: "server", method = ?method_name);
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
