use std::net::SocketAddr;
use std::sync::Arc;

use config::RpcConfig;
use jsonrpsee::core::Error;
use jsonrpsee::server::{ServerBuilder, ServerHandle};
use katana::api::KatanaApiServer;
use katana::KatanaRpc;
use katana_core::sequencer::Sequencer;
use tokio::sync::RwLock;

pub mod config;
mod katana;
mod starknet;
mod utils;

use self::starknet::api::{StarknetApiError, StarknetApiServer};
use self::starknet::StarknetRpc;

#[derive(Debug, Clone)]
pub struct KatanaNodeRpc<S> {
    pub config: RpcConfig,
    pub sequencer: Arc<RwLock<S>>,
}

impl<S> KatanaNodeRpc<S>
where
    S: Sequencer + Send + Sync + 'static,
{
    pub fn new(sequencer: Arc<RwLock<S>>, config: RpcConfig) -> Self {
        Self { config, sequencer }
    }

    pub async fn run(self) -> Result<(SocketAddr, ServerHandle), Error> {
        let mut methods = KatanaRpc::new(self.sequencer.clone()).into_rpc();
        methods.merge(StarknetRpc::new(self.sequencer.clone()).into_rpc())?;

        let server = ServerBuilder::new()
            .set_logger(KatanaNodeRpcLogger)
            .build(format!("127.0.0.1:{}", self.config.port))
            .await
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let addr = server.local_addr()?;
        let handle = server.start(methods)?;

        Ok((addr, handle))
    }
}

use std::time::Instant;

use jsonrpsee::server::logger::{Logger, MethodKind, TransportProtocol};
use jsonrpsee::tracing::info;
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
        info!("method: '{}'", method_name);
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
