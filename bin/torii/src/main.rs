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
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use dojo_metrics::exporters::prometheus::PrometheusRecorder;
use dojo_utils::parse::parse_url;
use dojo_world::contracts::world::WorldContractReader;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{
    SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::SqlitePool;
use starknet::core::types::Felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tempfile::NamedTempFile;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tokio_stream::StreamExt;
use torii_core::engine::{Engine, EngineConfig, IndexingFlags, Processors};
use torii_core::executor::Executor;
use torii_core::processors::store_transaction::StoreTransactionProcessor;
use torii_core::processors::EventProcessorConfig;
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::cache::ModelCache;
use torii_core::sql::Sql;
use torii_core::types::{Contract, ContractType, Model};
use torii_server::proxy::Proxy;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
use url::{form_urlencoded, Url};

mod options;

use options::*;

pub(crate) const LOG_TARGET: &str = "torii::cli";

const DEFAULT_RPC_URL: &str = "http://0.0.0.0:5050";

/// Dojo World Indexer
#[derive(Parser, Debug)]
#[command(name = "torii", author, version, about, long_about = None)]
struct ToriiArgs {
    /// The world to index
    #[arg(short, long = "world", env = "DOJO_WORLD_ADDRESS")]
    world_address: Option<Felt>,

    /// The sequencer rpc endpoint to index.
    #[arg(long, value_name = "URL", default_value = DEFAULT_RPC_URL, value_parser = parse_url)]
    rpc: Url,

    /// Database filepath (ex: indexer.db). If specified file doesn't exist, it will be
    /// created. Defaults to in-memory database.
    #[arg(long)]
    #[arg(
        value_name = "PATH",
        help = "Database filepath. If specified directory doesn't exist, it will be created. \
                Defaults to in-memory database."
    )]
    db_dir: Option<PathBuf>,

    /// The external url of the server, used for configuring the GraphQL Playground in a hosted
    /// environment
    #[arg(long, value_parser = parse_url, help = "The external url of the server, used for configuring the GraphQL Playground in a hosted environment.")]
    external_url: Option<Url>,

    /// Open World Explorer on the browser.
    #[arg(long, help = "Open World Explorer on the browser.")]
    explorer: bool,

    #[command(flatten)]
    metrics: MetricsOptions,

    #[command(flatten)]
    indexing: IndexingOptions,

    #[command(flatten)]
    events: EventsOptions,

    #[command(flatten)]
    server: ServerOptions,

    #[command(flatten)]
    relay: RelayOptions,

    /// Configuration file
    #[arg(long, help = "Configuration file to setup Torii.")]
    config: Option<PathBuf>,
}

impl ToriiArgs {
    pub fn with_config_file(mut self) -> Result<Self> {
        let config: ToriiArgsConfig = if let Some(path) = &self.config {
            toml::from_str(&std::fs::read_to_string(path)?)?
        } else {
            return Ok(self);
        };

        // the CLI (self) takes precedence over the config file.
        // Currently, the merge is made at the top level of the commands.
        // We may add recursive merging in the future.

        if self.world_address.is_none() {
            self.world_address = config.world_address;
        }

        if self.rpc == Url::parse(DEFAULT_RPC_URL).unwrap() {
            if let Some(rpc) = config.rpc {
                self.rpc = rpc;
            }
        }

        if self.db_dir.is_none() {
            self.db_dir = config.db_dir;
        }

        if self.external_url.is_none() {
            self.external_url = config.external_url;
        }

        // Currently the comparison it's only at the top level.
        // Need to make it more granular.

        if !self.explorer {
            self.explorer = config.explorer.unwrap_or_default();
        }

        if self.metrics == MetricsOptions::default() {
            self.metrics = config.metrics.unwrap_or_default();
        }

        if self.indexing == IndexingOptions::default() {
            self.indexing = config.indexing.unwrap_or_default();
        }

        if self.events == EventsOptions::default() {
            self.events = config.events.unwrap_or_default();
        }

        if self.server == ServerOptions::default() {
            self.server = config.server.unwrap_or_default();
        }

        if self.relay == RelayOptions::default() {
            self.relay = config.relay.unwrap_or_default();
        }

        Ok(self)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ToriiArgsConfig {
    pub world_address: Option<Felt>,
    pub rpc: Option<Url>,
    pub db_dir: Option<PathBuf>,
    pub external_url: Option<Url>,
    pub explorer: Option<bool>,
    pub metrics: Option<MetricsOptions>,
    pub indexing: Option<IndexingOptions>,
    pub events: Option<EventsOptions>,
    pub server: Option<ServerOptions>,
    pub relay: Option<RelayOptions>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = ToriiArgs::parse().with_config_file()?;

    let world_address = if let Some(world_address) = args.world_address {
        world_address
    } else {
        return Err(anyhow::anyhow!("Please specify a world address."));
    };

    // let mut contracts = parse_erc_contracts(&args.contracts)?;
    args.indexing.contracts.push(Contract { address: world_address, r#type: ContractType::WORLD });

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
    let database_path =
        if let Some(db_dir) = args.db_dir { db_dir } else { tempfile.path().to_path_buf() };

    let mut options = SqliteConnectOptions::from_str(&database_path.to_string_lossy())?
        .create_if_missing(true)
        .with_regexp();

    // Performance settings
    options = options.auto_vacuum(SqliteAutoVacuum::None);
    options = options.journal_mode(SqliteJournalMode::Wal);
    options = options.synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new().min_connections(1).connect_with(options).await?;

    // Set the number of threads based on CPU count
    let cpu_count = std::thread::available_parallelism().unwrap().get();
    let thread_count = cmp::min(cpu_count, 8);
    sqlx::query(&format!("PRAGMA threads = {};", thread_count)).execute(&pool).await?;

    sqlx::migrate!("../../crates/torii/migrations").run(&pool).await?;

    let provider: Arc<_> = JsonRpcClient::new(HttpTransport::new(args.rpc)).into();

    // Get world address
    let world = WorldContractReader::new(world_address, provider.clone());

    let (mut executor, sender) = Executor::new(pool.clone(), shutdown_tx.clone()).await?;
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &args.contracts, model_cache.clone()).await?;

    let processors = Processors {
        transaction: vec![Box::new(StoreTransactionProcessor)],
        ..Processors::default()
    };

    let (block_tx, block_rx) = tokio::sync::mpsc::channel(100);

    let mut flags = IndexingFlags::empty();
    if args.indexing.index_transactions {
        flags.insert(IndexingFlags::TRANSACTIONS);
    }
    if args.events.raw {
        flags.insert(IndexingFlags::RAW_EVENTS);
    }

    let mut engine: Engine<Arc<JsonRpcClient<HttpTransport>>> = Engine::new(
        world,
        db.clone(),
        provider.clone(),
        processors,
        EngineConfig {
            max_concurrent_tasks: args.indexing.max_concurrent_tasks,
            start_block: 0,
            blocks_chunk_size: args.indexing.blocks_chunk_size,
            events_chunk_size: args.indexing.events_chunk_size,
            index_pending: args.indexing.index_pending,
            polling_interval: Duration::from_millis(args.indexing.polling_interval),
            flags,
            event_processor_config: EventProcessorConfig {
                historical_events: args.events.historical.unwrap_or_default().into_iter().collect(),
            },
        },
        shutdown_tx.clone(),
        Some(block_tx),
        &args.indexing.contracts,
    );

    let shutdown_rx = shutdown_tx.subscribe();
    let (grpc_addr, grpc_server) = torii_grpc::server::new(
        shutdown_rx,
        &pool,
        block_rx,
        world_address,
        Arc::clone(&provider),
        model_cache,
    )
    .await?;

    let mut libp2p_relay_server = torii_relay::server::Relay::new(
        db,
        provider.clone(),
        args.relay.port,
        args.relay.webrtc_port,
        args.relay.websocket_port,
        args.relay.local_key_path,
        args.relay.cert_path,
    )
    .expect("Failed to start libp2p relay server");

    let addr = SocketAddr::new(args.server.http_addr, args.server.http_port);
    let proxy_server = Arc::new(Proxy::new(
        addr,
        args.server.http_cors_origins.filter(|cors_origins| !cors_origins.is_empty()),
        Some(grpc_addr),
        None,
    ));

    let graphql_server = spawn_rebuilding_graphql_server(
        shutdown_tx.clone(),
        pool.into(),
        args.external_url,
        proxy_server.clone(),
    );

    let gql_endpoint = format!("{addr}/graphql");
    let encoded: String =
        form_urlencoded::byte_serialize(gql_endpoint.replace("0.0.0.0", "localhost").as_bytes())
            .collect();
    let explorer_url = format!("https://worlds.dev/torii?url={}", encoded);
    info!(target: LOG_TARGET, endpoint = %addr, "Starting torii endpoint.");
    info!(target: LOG_TARGET, endpoint = %gql_endpoint, "Serving Graphql playground.");
    info!(target: LOG_TARGET, url = %explorer_url, "Serving World Explorer.");

    if args.explorer {
        if let Err(e) = webbrowser::open(&explorer_url) {
            error!(target: LOG_TARGET, error = %e, "Opening World Explorer in the browser.");
        }
    }

    if args.metrics.metrics {
        let addr = SocketAddr::new(args.metrics.metrics_addr, args.metrics.metrics_port);
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
    let libp2p_relay_server_handle = tokio::spawn(async move { libp2p_relay_server.run().await });

    tokio::select! {
        res = engine_handle => res??,
        res = proxy_server_handle => res??,
        res = graphql_server_handle => res?,
        res = grpc_server_handle => res??,
        res = libp2p_relay_server_handle => res?,
        _ = dojo_utils::signal::wait_signals() => {},
    };

    Ok(())
}

async fn spawn_rebuilding_graphql_server(
    shutdown_tx: Sender<()>,
    pool: Arc<SqlitePool>,
    external_url: Option<Url>,
    proxy_server: Arc<Proxy>,
) {
    let mut broker = SimpleBroker::<Model>::subscribe();

    loop {
        let shutdown_rx = shutdown_tx.subscribe();
        let (new_addr, new_server) =
            torii_graphql::server::new(shutdown_rx, &pool, external_url.clone()).await;

        tokio::spawn(new_server);

        proxy_server.set_graphql_addr(new_addr).await;

        // Break the loop if there are no more events
        if broker.next().await.is_none() {
            break;
        }
    }
}

#[cfg(test)]
mod test {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    #[test]
    fn test_cli_precedence() {
        // CLI args must take precedence over the config file.
        let content = r#"
        world_address = "0x1234"
        rpc = "http://0.0.0.0:5050"
        db_dir = "/tmp/torii-test"
        
        [events]
        raw = true
        historical = [
            "ns-E",
            "ns-EH"
        ]
        "#;
        let path = std::env::temp_dir().join("torii-config2.json");
        std::fs::write(&path, content).unwrap();

        let path_str = path.to_string_lossy().to_string();

        let args = vec![
            "torii",
            "--world",
            "0x9999",
            "--rpc",
            "http://0.0.0.0:6060",
            "--db-dir",
            "/tmp/torii-test2",
            "--events.raw",
            "false",
            "--events.historical",
            "a-A",
            "--config",
            path_str.as_str(),
        ];

        let torii_args = ToriiArgs::parse_from(args).with_config_file().unwrap();

        assert_eq!(torii_args.world_address, Some(Felt::from_str("0x9999").unwrap()));
        assert_eq!(torii_args.rpc, Url::parse("http://0.0.0.0:6060").unwrap());
        assert_eq!(torii_args.db_dir, Some(PathBuf::from("/tmp/torii-test2")));
        assert!(!torii_args.events.raw);
        assert_eq!(torii_args.events.historical, Some(vec!["a-A".to_string()]));
        assert_eq!(torii_args.server, ServerOptions::default());
    }

    #[test]
    fn test_config_fallback() {
        let content = r#"
        world_address = "0x1234"
        rpc = "http://0.0.0.0:2222"
        db_dir = "/tmp/torii-test"

        [events]
        raw = false
        historical = [
            "ns-E",
            "ns-EH"
        ]

        [server]
        http_addr = "127.0.0.1"
        http_port = 7777
        http_cors_origins = ["*"]

        [indexing]
        events_chunk_size = 9999
        index_pending = true
        max_concurrent_tasks = 1000
        index_transactions = false
        contracts = [
            "erc20:0x1234",
            "erc721:0x5678"
        ]
        "#;
        let path = std::env::temp_dir().join("torii-config.json");
        std::fs::write(&path, content).unwrap();

        let path_str = path.to_string_lossy().to_string();

        let args = vec!["torii", "--config", path_str.as_str()];

        let torii_args = ToriiArgs::parse_from(args).with_config_file().unwrap();

        assert_eq!(torii_args.world_address, Some(Felt::from_str("0x1234").unwrap()));
        assert_eq!(torii_args.rpc, Url::parse("http://0.0.0.0:2222").unwrap());
        assert_eq!(torii_args.db_dir, Some(PathBuf::from("/tmp/torii-test")));
        assert!(!torii_args.events.raw);
        assert_eq!(
            torii_args.events.historical,
            Some(vec!["ns-E".to_string(), "ns-EH".to_string()])
        );
        assert_eq!(torii_args.indexing.events_chunk_size, 9999);
        assert_eq!(torii_args.indexing.blocks_chunk_size, 10240);
        assert!(torii_args.indexing.index_pending);
        assert_eq!(torii_args.indexing.polling_interval, 500);
        assert_eq!(torii_args.indexing.max_concurrent_tasks, 1000);
        assert!(!torii_args.indexing.index_transactions);
        assert_eq!(
            torii_args.indexing.contracts,
            vec![
                Contract {
                    address: Felt::from_str("0x1234").unwrap(),
                    r#type: ContractType::ERC20
                },
                Contract {
                    address: Felt::from_str("0x5678").unwrap(),
                    r#type: ContractType::ERC721
                }
            ]
        );
        assert_eq!(torii_args.server.http_addr, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(torii_args.server.http_port, 7777);
        assert_eq!(torii_args.server.http_cors_origins, Some(vec!["*".to_string()]));
    }
}
