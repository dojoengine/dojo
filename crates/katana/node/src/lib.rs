#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use dojo_metrics::{metrics_process, prometheus_exporter, Report};
use hyper::{Method, Uri};
use jsonrpsee::server::middleware::proxy_get_request::ProxyGetRequestLayer;
use jsonrpsee::server::{AllowHosts, ServerBuilder, ServerHandle};
use jsonrpsee::RpcModule;
use katana_core::backend::config::StarknetConfig;
use katana_core::backend::storage::Blockchain;
use katana_core::backend::Backend;
use katana_core::constants::MAX_RECURSION_DEPTH;
use katana_core::env::BlockContextGenerator;
#[allow(deprecated)]
use katana_core::sequencer::SequencerConfig;
use katana_core::service::block_producer::BlockProducer;
#[cfg(feature = "messaging")]
use katana_core::service::messaging::{MessagingService, MessagingTask};
use katana_core::service::{BlockProductionTask, TransactionMiner};
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::{ExecutorFactory, SimulationFlag};
use katana_pool::ordering::FiFo;
use katana_pool::validation::stateful::TxValidator;
use katana_pool::{TransactionPool, TxPool};
use katana_primitives::block::FinalityStatus;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_rpc::config::ServerConfig;
use katana_rpc::dev::DevApi;
use katana_rpc::metrics::RpcServerMetrics;
use katana_rpc::saya::SayaApi;
use katana_rpc::starknet::StarknetApi;
use katana_rpc::torii::ToriiApi;
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_api::saya::SayaApiServer;
use katana_rpc_api::starknet::{StarknetApiServer, StarknetTraceApiServer, StarknetWriteApiServer};
use katana_rpc_api::torii::ToriiApiServer;
use katana_rpc_api::ApiKind;
use katana_tasks::TaskManager;
use num_traits::ToPrimitive;
use starknet::core::types::{BlockId, BlockStatus, MaybePendingBlockWithTxHashes};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{info, trace};

/// A handle to the instantiated Katana node.
#[allow(missing_debug_implementations)]
pub struct Handle {
    pub pool: TxPool,
    pub rpc: RpcServer,
    pub task_manager: TaskManager,
    pub backend: Arc<Backend<BlockifierFactory>>,
    pub block_producer: Arc<BlockProducer<BlockifierFactory>>,
}

impl Handle {
    /// Stops the Katana node.
    pub async fn stop(self) -> Result<()> {
        // TODO: wait for the rpc server to stop
        self.rpc.handle.stop()?;
        self.task_manager.shutdown().await;
        Ok(())
    }
}

/// Build the core Katana components from the given configurations and start running the node.
// TODO: placeholder until we implement a dedicated class that encapsulate building the node
// components
//
// Most of the logic are taken out of the `main.rs` file in `/bin/katana` crate, and combined
// with the exact copy of the setup logic for `NodeService` from `KatanaSequencer::new`. It also
// includes logic that was previously in `Backend::new`.
//
// NOTE: Don't rely on this function as it is mainly used as a placeholder for now.
#[allow(deprecated)]
pub async fn start(
    server_config: ServerConfig,
    sequencer_config: SequencerConfig,
    mut starknet_config: StarknetConfig,
) -> Result<Handle> {
    // --- build executor factory

    let cfg_env = CfgEnv {
        chain_id: starknet_config.env.chain_id,
        invoke_tx_max_n_steps: starknet_config.env.invoke_max_steps,
        validate_max_n_steps: starknet_config.env.validate_max_steps,
        max_recursion_depth: MAX_RECURSION_DEPTH,
        fee_token_addresses: FeeTokenAddressses {
            eth: starknet_config.genesis.fee_token.address,
            strk: Default::default(),
        },
    };

    let simulation_flags = SimulationFlag {
        skip_validate: starknet_config.disable_validate,
        skip_fee_transfer: starknet_config.disable_fee,
        ..Default::default()
    };

    let executor_factory = Arc::new(BlockifierFactory::new(cfg_env, simulation_flags));

    // --- build backend

    let (blockchain, db) = if let Some(forked_url) = &starknet_config.fork_rpc_url {
        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(forked_url.clone())));
        let forked_chain_id = provider.chain_id().await.unwrap();

        let forked_block_num = if let Some(num) = starknet_config.fork_block_number {
            num
        } else {
            provider.block_number().await.expect("failed to fetch block number from forked network")
        };

        let block =
            provider.get_block_with_tx_hashes(BlockId::Number(forked_block_num)).await.unwrap();
        let MaybePendingBlockWithTxHashes::Block(block) = block else {
            panic!("block to be forked is a pending block")
        };

        // adjust the genesis to match the forked block
        starknet_config.genesis.number = block.block_number;
        starknet_config.genesis.state_root = block.new_root;
        starknet_config.genesis.parent_hash = block.parent_hash;
        starknet_config.genesis.timestamp = block.timestamp;
        starknet_config.genesis.sequencer_address = block.sequencer_address.into();
        starknet_config.genesis.gas_prices.eth =
            block.l1_gas_price.price_in_wei.to_u128().expect("should fit in u128");
        starknet_config.genesis.gas_prices.strk =
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
            &starknet_config.genesis,
            match block.status {
                BlockStatus::AcceptedOnL1 => FinalityStatus::AcceptedOnL1,
                BlockStatus::AcceptedOnL2 => FinalityStatus::AcceptedOnL2,
                _ => panic!("unable to fork for non-accepted block"),
            },
        )?;

        starknet_config.env.chain_id = forked_chain_id.into();

        (blockchain, None)
    } else if let Some(db_path) = &starknet_config.db_dir {
        let db = katana_db::init_db(db_path)?;
        (Blockchain::new_with_db(db.clone(), &starknet_config.genesis)?, Some(db))
    } else {
        (Blockchain::new_with_genesis(InMemoryProvider::new(), &starknet_config.genesis)?, None)
    };

    let chain_id = starknet_config.env.chain_id;
    let block_context_generator = BlockContextGenerator::default().into();
    let backend = Arc::new(Backend {
        chain_id,
        blockchain,
        executor_factory,
        block_context_generator,
        config: starknet_config,
    });

    // --- build block producer

    let block_producer = if sequencer_config.block_time.is_some() || sequencer_config.no_mining {
        if let Some(interval) = sequencer_config.block_time {
            BlockProducer::interval(Arc::clone(&backend), interval)
        } else {
            BlockProducer::on_demand(Arc::clone(&backend))
        }
    } else {
        BlockProducer::instant(Arc::clone(&backend))
    };

    // --- build transaction pool and miner

    let validator = block_producer.validator();
    let pool = TxPool::new(validator.clone(), FiFo::new());
    let miner = TransactionMiner::new(pool.add_listener());

    // --- build metrics service

    // Metrics recorder must be initialized before calling any of the metrics macros, in order for
    // it to be registered.
    if let Some(addr) = server_config.metrics {
        let prometheus_handle = prometheus_exporter::install_recorder("katana")?;
        let reports = db.map(|db| vec![Box::new(db) as Box<dyn Report>]).unwrap_or_default();

        prometheus_exporter::serve(
            addr,
            prometheus_handle,
            metrics_process::Collector::default(),
            reports,
        )
        .await?;

        info!(%addr, "Metrics endpoint started.");
    }

    // --- create a TaskManager using the ambient Tokio runtime

    let task_manager = TaskManager::current();

    // --- build and spawn the messaging task

    #[cfg(feature = "messaging")]
    if let Some(config) = sequencer_config.messaging.clone() {
        let messaging = MessagingService::new(config, pool.clone(), Arc::clone(&backend)).await?;
        let task = MessagingTask::new(messaging);
        task_manager.build_task().critical().name("Messaging").spawn(task);
    }

    let block_producer = Arc::new(block_producer);

    // --- build and spawn the block production task

    let task = BlockProductionTask::new(pool.clone(), miner, block_producer.clone());
    task_manager.build_task().critical().name("BlockProduction").spawn(task);

    // --- spawn rpc server

    let node_components = (pool.clone(), backend.clone(), block_producer.clone(), validator);
    let rpc = spawn(node_components, server_config).await?;

    Ok(Handle { backend, block_producer, pool, rpc, task_manager })
}

// Moved from `katana_rpc` crate
pub async fn spawn<EF: ExecutorFactory>(
    node_components: (TxPool, Arc<Backend<EF>>, Arc<BlockProducer<EF>>, TxValidator),
    config: ServerConfig,
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
        .layer(ProxyGetRequestLayer::new("/account_balance", "dev_accountBalance")?)
        .layer(ProxyGetRequestLayer::new("/fee_token", "dev_feeToken")?)
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

    Ok(RpcServer { handle, addr })
}

#[derive(Debug)]
pub struct RpcServer {
    pub addr: SocketAddr,
    pub handle: ServerHandle,
}
