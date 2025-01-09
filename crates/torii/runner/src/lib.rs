//! Torii binary executable.
//!
//! ## Feature Flags
//!
//! - `jemalloc`: Uses [jemallocator](https://github.com/tikv/jemallocator) as the global allocator.
//!   This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.
//! - `jemalloc-prof`: Enables [jemallocator's](https://github.com/tikv/jemallocator) heap profiling
//!   and leak detection functionality. See [jemalloc's opt.prof](https://jemalloc.net/jemalloc.3.html#opt.prof)
//!   documentation for usage details. This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.

use std::cmp;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use camino::Utf8PathBuf;
use dojo_metrics::exporters::prometheus::PrometheusRecorder;
use dojo_world::contracts::world::WorldContractReader;
use sqlx::sqlite::{
    SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::SqlitePool;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tempfile::{NamedTempFile, TempDir};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tokio_stream::StreamExt;
use torii_cli::ToriiArgs;
use torii_indexer::engine::{Engine, EngineConfig, IndexingFlags, Processors};
use torii_indexer::processors::store_transaction::StoreTransactionProcessor;
use torii_indexer::processors::EventProcessorConfig;
use torii_server::proxy::Proxy;
use torii_sqlite::cache::ModelCache;
use torii_sqlite::executor::Executor;
use torii_sqlite::simple_broker::SimpleBroker;
use torii_sqlite::types::{Contract, ContractType, Model};
use torii_sqlite::Sql;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
use url::form_urlencoded;

pub(crate) const LOG_TARGET: &str = "torii_runner";

pub struct Runner {
    args: ToriiArgs,
}

impl Runner {
    pub fn new(args: ToriiArgs) -> Self {
        Self { args }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let world_address = if let Some(world_address) = self.args.world_address {
            world_address
        } else {
            return Err(anyhow::anyhow!("Please specify a world address."));
        };

        self.args
            .indexing
            .contracts
            .push(Contract { address: world_address, r#type: ContractType::WORLD });

        let filter_layer = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,hyper_reverse_proxy=off"));

        let subscriber = fmt::Subscriber::builder().with_env_filter(filter_layer).finish();

        // Set the global subscriber
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set the global tracing subscriber");

        // Setup cancellation for graceful shutdown
        let (shutdown_tx, _) = broadcast::channel(1);

        let shutdown_tx_clone = shutdown_tx.clone();
        ctrlc::set_handler(move || {
            let _ = shutdown_tx_clone.send(());
        })
        .expect("Error setting Ctrl-C handler");

        let tempfile = NamedTempFile::new()?;
        let database_path = if let Some(db_dir) = self.args.db_dir {
            // Create the directory if it doesn't exist
            std::fs::create_dir_all(&db_dir)?;
            // Set the database file path inside the directory
            db_dir.join("torii.db")
        } else {
            tempfile.path().to_path_buf()
        };

        let mut options = SqliteConnectOptions::from_str(&database_path.to_string_lossy())?
            .create_if_missing(true)
            .with_regexp();

        // Performance settings
        options = options.auto_vacuum(SqliteAutoVacuum::None);
        options = options.journal_mode(SqliteJournalMode::Wal);
        options = options.synchronous(SqliteSynchronous::Normal);

        let pool =
            SqlitePoolOptions::new().min_connections(1).connect_with(options.clone()).await?;

        let readonly_options = options.read_only(true);
        let readonly_pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(100)
            .connect_with(readonly_options)
            .await?;

        // Set the number of threads based on CPU count
        let cpu_count = std::thread::available_parallelism().unwrap().get();
        let thread_count = cmp::min(cpu_count, 8);
        sqlx::query(&format!("PRAGMA threads = {};", thread_count)).execute(&pool).await?;

        sqlx::migrate!("../migrations").run(&pool).await?;

        let provider: Arc<_> = JsonRpcClient::new(HttpTransport::new(self.args.rpc)).into();

        // Get world address
        let world = WorldContractReader::new(world_address, provider.clone());

        let (mut executor, sender) = Executor::new(
            pool.clone(),
            shutdown_tx.clone(),
            provider.clone(),
            self.args.indexing.max_concurrent_tasks,
        )
        .await?;
        let executor_handle = tokio::spawn(async move { executor.run().await });

        let model_cache = Arc::new(ModelCache::new(readonly_pool.clone()));
        let db = Sql::new(
            pool.clone(),
            sender.clone(),
            &self.args.indexing.contracts,
            model_cache.clone(),
        )
        .await?;

        let processors = Processors {
            transaction: vec![Box::new(StoreTransactionProcessor)],
            ..Processors::default()
        };

        let (block_tx, block_rx) = tokio::sync::mpsc::channel(100);

        let mut flags = IndexingFlags::empty();
        if self.args.indexing.transactions {
            flags.insert(IndexingFlags::TRANSACTIONS);
        }
        if self.args.events.raw {
            flags.insert(IndexingFlags::RAW_EVENTS);
        }
        if self.args.indexing.pending {
            flags.insert(IndexingFlags::PENDING_BLOCKS);
        }

        let mut engine: Engine<Arc<JsonRpcClient<HttpTransport>>> = Engine::new(
            world,
            db.clone(),
            provider.clone(),
            processors,
            EngineConfig {
                max_concurrent_tasks: self.args.indexing.max_concurrent_tasks,
                blocks_chunk_size: self.args.indexing.blocks_chunk_size,
                events_chunk_size: self.args.indexing.events_chunk_size,
                polling_interval: Duration::from_millis(self.args.indexing.polling_interval),
                flags,
                event_processor_config: EventProcessorConfig {
                    historical_events: self.args.events.historical.into_iter().collect(),
                    namespaces: self.args.indexing.namespaces.into_iter().collect(),
                },
                world_block: self.args.indexing.world_block,
            },
            shutdown_tx.clone(),
            Some(block_tx),
            &self.args.indexing.contracts,
        );

        let shutdown_rx = shutdown_tx.subscribe();
        let (grpc_addr, grpc_server) = torii_grpc::server::new(
            shutdown_rx,
            &readonly_pool,
            block_rx,
            world_address,
            Arc::clone(&provider),
            model_cache,
        )
        .await?;

        let temp_dir = TempDir::new()?;
        let artifacts_path = self
            .args
            .artifacts_path
            .unwrap_or_else(|| Utf8PathBuf::from(temp_dir.path().to_str().unwrap()));

        tokio::fs::create_dir_all(&artifacts_path).await?;
        let absolute_path = artifacts_path.canonicalize_utf8()?;

        let (artifacts_addr, artifacts_server) = torii_server::artifacts::new(
            shutdown_tx.subscribe(),
            &absolute_path,
            readonly_pool.clone(),
        )
        .await?;

        let mut libp2p_relay_server = torii_relay::server::Relay::new(
            db,
            provider.clone(),
            self.args.relay.port,
            self.args.relay.webrtc_port,
            self.args.relay.websocket_port,
            self.args.relay.local_key_path,
            self.args.relay.cert_path,
        )
        .expect("Failed to start libp2p relay server");

        let addr = SocketAddr::new(self.args.server.http_addr, self.args.server.http_port);

        let proxy_server = Arc::new(Proxy::new(
            addr,
            self.args.server.http_cors_origins.filter(|cors_origins| !cors_origins.is_empty()),
            Some(grpc_addr),
            None,
            Some(artifacts_addr),
            Arc::new(readonly_pool.clone()),
        ));

        let graphql_server = spawn_rebuilding_graphql_server(
            shutdown_tx.clone(),
            readonly_pool.into(),
            proxy_server.clone(),
        );

        let gql_endpoint = format!("{addr}/graphql");
        let encoded: String = form_urlencoded::byte_serialize(
            gql_endpoint.replace("0.0.0.0", "localhost").as_bytes(),
        )
        .collect();
        let explorer_url = format!("https://worlds.dev/torii?url={}", encoded);
        info!(target: LOG_TARGET, endpoint = %addr, "Starting torii endpoint.");
        info!(target: LOG_TARGET, endpoint = %gql_endpoint, "Serving Graphql playground.");
        info!(target: LOG_TARGET, url = %explorer_url, "Serving World Explorer.");
        info!(target: LOG_TARGET, path = %artifacts_path, "Serving ERC artifacts at path");

        if self.args.explorer {
            if let Err(e) = webbrowser::open(&explorer_url) {
                error!(target: LOG_TARGET, error = %e, "Opening World Explorer in the browser.");
            }
        }

        if self.args.metrics.metrics {
            let addr =
                SocketAddr::new(self.args.metrics.metrics_addr, self.args.metrics.metrics_port);
            info!(target: LOG_TARGET, %addr, "Starting metrics endpoint.");
            let prometheus_handle = PrometheusRecorder::install("torii")?;
            let server = dojo_metrics::Server::new(prometheus_handle).with_process_metrics();
            tokio::spawn(server.start(addr));
        }

        let engine_handle = tokio::spawn(async move { engine.start().await });
        let proxy_server_handle =
            tokio::spawn(async move { proxy_server.start(shutdown_tx.subscribe()).await });
        let graphql_server_handle = tokio::spawn(graphql_server);
        let grpc_server_handle = tokio::spawn(grpc_server);
        let libp2p_relay_server_handle =
            tokio::spawn(async move { libp2p_relay_server.run().await });
        let artifacts_server_handle = tokio::spawn(artifacts_server);

        tokio::select! {
            res = engine_handle => res??,
            res = executor_handle => res??,
            res = proxy_server_handle => res??,
            res = graphql_server_handle => res?,
            res = grpc_server_handle => res??,
            res = libp2p_relay_server_handle => res?,
            res = artifacts_server_handle => res?,
            _ = dojo_utils::signal::wait_signals() => {},
        };

        Ok(())
    }
}

async fn spawn_rebuilding_graphql_server(
    shutdown_tx: Sender<()>,
    pool: Arc<SqlitePool>,
    proxy_server: Arc<Proxy>,
) {
    let mut broker = SimpleBroker::<Model>::subscribe();

    loop {
        let shutdown_rx = shutdown_tx.subscribe();
        let (new_addr, new_server) = torii_graphql::server::new(shutdown_rx, &pool).await;

        tokio::spawn(new_server);

        proxy_server.set_graphql_addr(new_addr).await;

        // Break the loop if there are no more events
        if broker.next().await.is_none() {
            break;
        } else {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
