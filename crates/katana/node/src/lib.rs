#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod config;
pub mod exit;

use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use config::metrics::MetricsConfig;
use config::rpc::{ApiKind, RpcConfig};
use config::{Config, SequencingConfig};
use dojo_metrics::prometheus_exporter::PrometheusHandle;
use dojo_metrics::{metrics_process, prometheus_exporter, Report};
use hyper::{Method, Uri};
use jsonrpsee::server::middleware::proxy_get_request::ProxyGetRequestLayer;
use jsonrpsee::server::{AllowHosts, ServerBuilder, ServerHandle};
use jsonrpsee::RpcModule;
use katana_core::backend::storage::Blockchain;
use katana_core::backend::Backend;
use katana_core::constants::MAX_RECURSION_DEPTH;
use katana_core::env::BlockContextGenerator;
use katana_core::service::block_producer::BlockProducer;
use katana_core::service::messaging::MessagingConfig;
use katana_db::mdbx::DbEnv;
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::{ExecutorFactory, SimulationFlag};
use katana_pipeline::{stage, Pipeline};
use katana_pool::ordering::FiFo;
use katana_pool::validation::stateful::TxValidator;
use katana_pool::TxPool;
use katana_primitives::block::FinalityStatus;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};
use katana_primitives::version::Version;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_rpc::dev::DevApi;
use katana_rpc::metrics::RpcServerMetrics;
use katana_rpc::saya::SayaApi;
use katana_rpc::starknet::StarknetApi;
use katana_rpc::torii::ToriiApi;
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_api::saya::SayaApiServer;
use katana_rpc_api::starknet::{StarknetApiServer, StarknetTraceApiServer, StarknetWriteApiServer};
use katana_rpc_api::torii::ToriiApiServer;
use katana_tasks::TaskManager;
use num_traits::ToPrimitive;
use starknet::core::types::{BlockId, BlockStatus, MaybePendingBlockWithTxHashes};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{info, trace};

use crate::exit::NodeStoppedFuture;

/// A handle to the launched node.
#[allow(missing_debug_implementations)]
pub struct LaunchedNode {
    pub node: Node,
    /// Handle to the rpc server.
    pub rpc: RpcServer,
}

impl LaunchedNode {
    /// Stops the node.
    ///
    /// This will instruct the node to stop and wait until it has actually stop.
    pub async fn stop(&self) -> Result<()> {
        // TODO: wait for the rpc server to stop instead of just stopping it.
        self.rpc.handle.stop()?;
        self.node.task_manager.shutdown().await;
        Ok(())
    }

    /// Returns a future which resolves only when the node has stopped.
    pub fn stopped(&self) -> NodeStoppedFuture<'_> {
        NodeStoppedFuture::new(self)
    }
}

/// A node instance.
///
/// The struct contains the handle to all the components of the node.
#[must_use = "Node does nothing unless launched."]
#[allow(missing_debug_implementations)]
pub struct Node {
    pub pool: TxPool,
    pub db: Option<DbEnv>,
    pub task_manager: TaskManager,
    pub prometheus_handle: PrometheusHandle,
    pub backend: Arc<Backend<BlockifierFactory>>,
    pub block_producer: BlockProducer<BlockifierFactory>,
    pub rpc_config: RpcConfig,
    pub metrics_config: Option<MetricsConfig>,
    pub sequencing_config: SequencingConfig,
    pub messaging_config: Option<MessagingConfig>,
}

impl Node {
    /// Start the node.
    ///
    /// This method will start all the node process, running them until the node is stopped.
    pub async fn launch(self) -> Result<LaunchedNode> {
        if let Some(ref cfg) = self.metrics_config {
            let addr = cfg.addr;
            let mut reports = Vec::new();

            if let Some(ref db) = self.db {
                reports.push(Box::new(db.clone()) as Box<dyn Report>);
            }

            prometheus_exporter::serve(
                addr,
                self.prometheus_handle.clone(),
                metrics_process::Collector::default(),
                reports,
            )
            .await?;

            info!(%addr, "Metrics endpoint started.");
        }

        let pool = self.pool.clone();
        let backend = self.backend.clone();
        let block_producer = self.block_producer.clone();
        let validator = self.block_producer.validator().clone();

        // --- build sequencing stage

        let sequencing = stage::Sequencing::new(
            pool.clone(),
            backend.clone(),
            self.task_manager.task_spawner(),
            block_producer.clone(),
            self.messaging_config.clone(),
        );

        // --- build and start the pipeline

        let mut pipeline = Pipeline::new();
        pipeline.add_stage(Box::new(sequencing));

        self.task_manager
            .task_spawner()
            .build_task()
            .critical()
            .name("Pipeline")
            .spawn(pipeline.into_future());

        let node_components = (pool, backend, block_producer, validator);
        let rpc = spawn(node_components, self.rpc_config.clone()).await?;

        Ok(LaunchedNode { node: self, rpc })
    }
}

/// Build the node components from the given [`Config`].
///
/// This returns a [`Node`] instance which can be launched with the all the necessary components
/// configured.
pub async fn build(mut config: Config) -> Result<Node> {
    // Metrics recorder must be initialized before calling any of the metrics macros, in order
    // for it to be registered.
    let prometheus_handle = prometheus_exporter::install_recorder("katana")?;

    // --- build executor factory

    let cfg_env = CfgEnv {
        chain_id: config.chain.id,
        invoke_tx_max_n_steps: config.starknet.env.invoke_max_steps,
        validate_max_n_steps: config.starknet.env.validate_max_steps,
        max_recursion_depth: MAX_RECURSION_DEPTH,
        fee_token_addresses: FeeTokenAddressses {
            eth: config.chain.fee_contracts.eth,
            strk: config.chain.fee_contracts.strk,
        },
    };

    let simulation_flags = SimulationFlag {
        skip_validate: !config.dev.account_validation,
        skip_fee_transfer: !config.dev.fee,
        ..Default::default()
    };

    let executor_factory = Arc::new(BlockifierFactory::new(cfg_env, simulation_flags));

    // --- build backend

    let (blockchain, db) = if let Some(forked_url) = &config.starknet.fork_rpc_url {
        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(forked_url.clone())));
        let forked_chain_id = provider.chain_id().await.unwrap();

        let forked_block_num = if let Some(num) = config.starknet.fork_block_number {
            num
        } else {
            provider.block_number().await.expect("failed to fetch block number from forked network")
        };

        let block =
            provider.get_block_with_tx_hashes(BlockId::Number(forked_block_num)).await.unwrap();
        let MaybePendingBlockWithTxHashes::Block(block) = block else {
            panic!("block to be forked is a pending block")
        };

        config.chain.version = Version::parse(&block.starknet_version)?;

        // adjust the genesis to match the forked block
        config.chain.genesis.number = block.block_number;
        config.chain.genesis.state_root = block.new_root;
        config.chain.genesis.parent_hash = block.parent_hash;
        config.chain.genesis.timestamp = block.timestamp;
        config.chain.genesis.sequencer_address = block.sequencer_address.into();
        config.chain.genesis.gas_prices.eth =
            block.l1_gas_price.price_in_wei.to_u128().expect("should fit in u128");
        config.chain.genesis.gas_prices.strk =
            block.l1_gas_price.price_in_fri.to_u128().expect("should fit in u128");

        trace!(
            chain = %parse_cairo_short_string(&forked_chain_id).unwrap(),
            block_number = %block.block_number,
            forked_url = %forked_url,
            "Forking chain.",
        );

        let blockchain = Blockchain::new_from_forked(
            ForkedProvider::new(provider, forked_block_num.into()).unwrap(),
            block.block_hash,
            &config.chain,
            match block.status {
                BlockStatus::AcceptedOnL1 => FinalityStatus::AcceptedOnL1,
                BlockStatus::AcceptedOnL2 => FinalityStatus::AcceptedOnL2,
                _ => panic!("unable to fork for non-accepted block"),
            },
        )?;

        config.chain.id = forked_chain_id.into();

        (blockchain, None)
    } else if let Some(db_path) = &config.db.dir {
        let db = katana_db::init_db(db_path)?;
        (Blockchain::new_with_db(db.clone(), &config.chain)?, Some(db))
    } else {
        (Blockchain::new_with_genesis(InMemoryProvider::new(), &config.chain)?, None)
    };

    let block_context_generator = BlockContextGenerator::default().into();
    let backend = Arc::new(Backend {
        blockchain,
        executor_factory,
        block_context_generator,
        config: config.starknet,
        chain_spec: config.chain,
    });

    // --- build block producer

    let block_producer = if config.sequencing.block_time.is_some() || config.sequencing.no_mining {
        if let Some(interval) = config.sequencing.block_time {
            BlockProducer::interval(Arc::clone(&backend), interval)
        } else {
            BlockProducer::on_demand(Arc::clone(&backend))
        }
    } else {
        BlockProducer::instant(Arc::clone(&backend))
    };

    // --- build transaction pool

    let validator = block_producer.validator();
    let pool = TxPool::new(validator.clone(), FiFo::new());

    let node = Node {
        db,
        pool,
        backend,
        block_producer,
        prometheus_handle,
        rpc_config: config.rpc,
        metrics_config: config.metrics,
        messaging_config: config.messaging,
        sequencing_config: config.sequencing,
        task_manager: TaskManager::current(),
    };

    Ok(node)
}

// Moved from `katana_rpc` crate
pub async fn spawn<EF: ExecutorFactory>(
    node_components: (TxPool, Arc<Backend<EF>>, BlockProducer<EF>, TxValidator),
    config: RpcConfig,
) -> Result<RpcServer> {
    let (pool, backend, block_producer, validator) = node_components;

    let mut methods = RpcModule::new(());
    methods.register_method("health", |_, _| Ok(serde_json::json!({ "health": true })))?;

    for api in &config.apis {
        match api {
            ApiKind::Starknet => {
                // TODO: merge these into a single logic.
                let server = StarknetApi::new(
                    backend.clone(),
                    pool.clone(),
                    block_producer.clone(),
                    validator.clone(),
                );
                methods.merge(StarknetApiServer::into_rpc(server.clone()))?;
                methods.merge(StarknetWriteApiServer::into_rpc(server.clone()))?;
                methods.merge(StarknetTraceApiServer::into_rpc(server))?;
            }
            ApiKind::Dev => {
                methods.merge(DevApi::new(backend.clone(), block_producer.clone()).into_rpc())?;
            }
            ApiKind::Torii => {
                methods.merge(
                    ToriiApi::new(backend.clone(), pool.clone(), block_producer.clone()).into_rpc(),
                )?;
            }
            ApiKind::Saya => {
                methods.merge(SayaApi::new(backend.clone(), block_producer.clone()).into_rpc())?;
            }
        }
    }

    let cors = CorsLayer::new()
            // Allow `POST` when accessing the resource
            .allow_methods([Method::POST, Method::GET])
            .allow_headers([hyper::header::CONTENT_TYPE, "argent-client".parse().unwrap(), "argent-version".parse().unwrap()]);

    let cors =
        config.allowed_origins.clone().map(|allowed_origins| match allowed_origins.as_slice() {
            [origin] if origin == "*" => cors.allow_origin(AllowOrigin::mirror_request()),
            origins => cors.allow_origin(
                origins
                    .iter()
                    .map(|o| {
                        let _ = o.parse::<Uri>().expect("Invalid URI");

                        o.parse().expect("Invalid origin")
                    })
                    .collect::<Vec<_>>(),
            ),
        });

    let middleware = tower::ServiceBuilder::new()
        .option_layer(cors)
        .layer(ProxyGetRequestLayer::new("/", "health")?)
        .timeout(Duration::from_secs(20));

    let server = ServerBuilder::new()
        .set_logger(RpcServerMetrics::new(&methods))
        .set_host_filtering(AllowHosts::Any)
        .set_middleware(middleware)
        .max_connections(config.max_connections)
        .build(config.socket_addr())
        .await?;

    let addr = server.local_addr()?;
    let handle = server.start(methods)?;

    info!(target: "rpc", %addr, "RPC server started.");

    Ok(RpcServer { handle, addr })
}

#[derive(Debug)]
pub struct RpcServer {
    pub addr: SocketAddr,
    pub handle: ServerHandle,
}
