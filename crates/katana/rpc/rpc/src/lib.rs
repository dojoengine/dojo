#![allow(clippy::blocks_in_conditions)]

pub mod config;
pub mod dev;
pub mod katana;
pub mod metrics;
pub mod saya;
pub mod starknet;
pub mod torii;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use config::ServerConfig;
use hyper::Method;
use jsonrpsee::server::middleware::proxy_get_request::ProxyGetRequestLayer;
use jsonrpsee::server::{AllowHosts, ServerBuilder, ServerHandle};
use jsonrpsee::RpcModule;
use katana_core::sequencer::KatanaSequencer;
use katana_executor::ExecutorFactory;
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_api::katana::KatanaApiServer;
use katana_rpc_api::saya::SayaApiServer;
use katana_rpc_api::starknet::StarknetApiServer;
use katana_rpc_api::torii::ToriiApiServer;
use katana_rpc_api::ApiKind;
use metrics::RpcServerMetrics;
use tower_http::cors::{Any, CorsLayer};

use crate::dev::DevApi;
use crate::katana::KatanaApi;
use crate::saya::SayaApi;
use crate::starknet::StarknetApi;
use crate::torii::ToriiApi;

pub async fn spawn<EF: ExecutorFactory>(
    sequencer: Arc<KatanaSequencer<EF>>,
    config: ServerConfig,
) -> Result<NodeHandle> {
    let mut methods = RpcModule::new(());
    methods.register_method("health", |_, _| Ok(serde_json::json!({ "health": true })))?;

    for api in &config.apis {
        match api {
            ApiKind::Starknet => {
                methods.merge(StarknetApi::new(sequencer.clone()).into_rpc())?;
            }
            ApiKind::Katana => {
                methods.merge(KatanaApi::new(sequencer.clone()).into_rpc())?;
            }
            ApiKind::Dev => {
                methods.merge(DevApi::new(sequencer.clone()).into_rpc())?;
            }
            ApiKind::Torii => {
                methods.merge(ToriiApi::new(sequencer.clone()).into_rpc())?;
            }
            ApiKind::Saya => {
                methods.merge(SayaApi::new(sequencer.clone()).into_rpc())?;
            }
        }
    }

    let cors = CorsLayer::new()
            // Allow `POST` when accessing the resource
            .allow_methods([Method::POST, Method::GET])
            // Allow requests from any origin
            .allow_origin(Any)
            .allow_headers([hyper::header::CONTENT_TYPE]);

    let middleware = tower::ServiceBuilder::new()
        .layer(cors)
        .layer(ProxyGetRequestLayer::new("/", "health")?)
        .timeout(Duration::from_secs(20));

    let server = ServerBuilder::new()
        .set_logger(RpcServerMetrics::new(&methods))
        .set_host_filtering(AllowHosts::Any)
        .set_middleware(middleware)
        .max_connections(config.max_connections)
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
