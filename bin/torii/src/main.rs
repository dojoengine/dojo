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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{ArgAction, CommandFactory, FromArgMatches, Parser};
use clap_config::ClapConfig;
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
use torii_core::sql::Sql;
use torii_core::types::{Contract, ContractType, Model};
use torii_server::proxy::Proxy;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
use url::{form_urlencoded, Url};

pub(crate) const LOG_TARGET: &str = "torii::cli";

/// Dojo World Indexer
#[derive(ClapConfig, Parser, Debug)]
#[command(name = "torii", author, version, about, long_about = None)]
struct Args {
    /// The world to index
    #[arg(short, long = "world", env = "DOJO_WORLD_ADDRESS")]
    world_address: Option<Felt>,

    /// The sequencer rpc endpoint to index.
    #[arg(long, value_name = "URL", default_value = ":5050", value_parser = parse_url)]
    rpc: Url,

    #[command(flatten)]
    server: ServerOptions,

    /// Database filepath (ex: indexer.db). If specified file doesn't exist, it will be
    /// created. Defaults to in-memory database
    #[arg(short, long, default_value = "")]
    database: String,

    /// Port to serve Libp2p TCP & UDP Quic transports
    #[arg(long, value_name = "PORT", default_value = "9090")]
    relay_port: u16,

    /// Port to serve Libp2p WebRTC transport
    #[arg(long, value_name = "PORT", default_value = "9091")]
    relay_webrtc_port: u16,

    /// Port to serve Libp2p WebRTC transport
    #[arg(long, value_name = "PORT", default_value = "9092")]
    relay_websocket_port: u16,

    /// Path to a local identity key file. If not specified, a new identity will be generated
    #[arg(long, value_name = "PATH")]
    relay_local_key_path: Option<String>,

    /// Path to a local certificate file. If not specified, a new certificate will be generated
    /// for WebRTC connections
    #[arg(long, value_name = "PATH")]
    relay_cert_path: Option<String>,

    /// The external url of the server, used for configuring the GraphQL Playground in a hosted
    /// environment
    #[arg(long, value_parser = parse_url)]
    external_url: Option<Url>,

    #[command(flatten)]
    metrics: MetricsOptions,

    /// Open World Explorer on the browser.
    #[arg(long)]
    explorer: bool,

    /// Chunk size of the events page when indexing using events
    #[arg(long, default_value = "1024")]
    events_chunk_size: u64,

    /// Number of blocks to process before commiting to DB
    #[arg(long, default_value = "10240")]
    blocks_chunk_size: u64,

    /// Enable indexing pending blocks
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    index_pending: bool,

    /// Polling interval in ms
    #[arg(long, default_value = "500")]
    polling_interval: u64,

    /// Max concurrent tasks
    #[arg(long, default_value = "100")]
    max_concurrent_tasks: usize,

    /// Whether or not to index world transactions
    #[arg(long, action = ArgAction::Set, default_value_t = false)]
    index_transactions: bool,

    /// Whether or not to index raw events
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    index_raw_events: bool,

    /// ERC contract addresses to index
    #[arg(long, value_delimiter = ',', value_parser = parse_erc_contract)]
    contracts: Vec<Contract>,

    /// Event messages that are going to be treated as historical
    /// A list of the model tags (namespace-name)
    #[arg(long, value_delimiter = ',')]
    historical_events: Vec<String>,

    /// Configuration file
    #[arg(long)]
    #[clap_config(skip)]
    config: Option<PathBuf>,
}

/// Metrics server default address.
const DEFAULT_METRICS_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
/// Torii metrics server default port.
const DEFAULT_METRICS_PORT: u16 = 9200;

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize)]
#[command(next_help_heading = "Metrics options")]
struct MetricsOptions {
    /// Enable metrics.
    ///
    /// For now, metrics will still be collected even if this flag is not set. This only
    /// controls whether the metrics server is started or not.
    #[arg(long)]
    metrics: bool,

    /// The metrics will be served at the given address.
    #[arg(requires = "metrics")]
    #[arg(long = "metrics.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_METRICS_ADDR)]
    metrics_addr: IpAddr,

    /// The metrics will be served at the given port.
    #[arg(requires = "metrics")]
    #[arg(long = "metrics.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_METRICS_PORT)]
    metrics_port: u16,
}

const DEFAULT_HTTP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_HTTP_PORT: u16 = 8080;

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Server options")]
struct ServerOptions {
    /// HTTP server listening interface.
    #[arg(long = "http.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_HTTP_ADDR)]
    http_addr: IpAddr,

    /// HTTP server listening port.
    #[arg(long = "http.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_HTTP_PORT)]
    http_port: u16,

    /// Comma separated list of domains from which to accept cross origin requests.
    #[arg(long = "http.corsdomain")]
    #[arg(value_delimiter = ',')]
    http_cors_origins: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = <Args as CommandFactory>::command().get_matches();
    let mut args = if let Some(path) = matches.get_one::<PathBuf>("config") {
        let config: ArgsConfig = toml::from_str(&std::fs::read_to_string(path)?)?;
        Args::from_merged(matches, Some(config))
    } else {
        Args::from_arg_matches(&matches)?
    };

    let world_address = if let Some(world_address) = args.world_address {
        world_address
    } else {
        return Err(anyhow::anyhow!("Please specify a world address."));
    };

    // let mut contracts = parse_erc_contracts(&args.contracts)?;
    args.contracts.push(Contract { address: world_address, r#type: ContractType::WORLD });

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
        if args.database.is_empty() { tempfile.path().to_str().unwrap() } else { &args.database };

    let mut options =
        SqliteConnectOptions::from_str(database_path)?.create_if_missing(true).with_regexp();

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

    let db = Sql::new(pool.clone(), sender.clone(), &args.contracts).await?;

    let processors = Processors {
        transaction: vec![Box::new(StoreTransactionProcessor)],
        ..Processors::default()
    };

    let (block_tx, block_rx) = tokio::sync::mpsc::channel(100);

    let mut flags = IndexingFlags::empty();
    if args.index_transactions {
        flags.insert(IndexingFlags::TRANSACTIONS);
    }
    if args.index_raw_events {
        flags.insert(IndexingFlags::RAW_EVENTS);
    }

    let mut engine: Engine<Arc<JsonRpcClient<HttpTransport>>> = Engine::new(
        world,
        db.clone(),
        provider.clone(),
        processors,
        EngineConfig {
            max_concurrent_tasks: args.max_concurrent_tasks,
            start_block: 0,
            blocks_chunk_size: args.blocks_chunk_size,
            events_chunk_size: args.events_chunk_size,
            index_pending: args.index_pending,
            polling_interval: Duration::from_millis(args.polling_interval),
            flags,
            event_processor_config: EventProcessorConfig {
                historical_events: args.historical_events.into_iter().collect(),
            },
        },
        shutdown_tx.clone(),
        Some(block_tx),
        &args.contracts,
    );

    let shutdown_rx = shutdown_tx.subscribe();
    let (grpc_addr, grpc_server) =
        torii_grpc::server::new(shutdown_rx, &pool, block_rx, world_address, Arc::clone(&provider))
            .await?;

    let mut libp2p_relay_server = torii_relay::server::Relay::new(
        db,
        provider.clone(),
        args.relay_port,
        args.relay_webrtc_port,
        args.relay_websocket_port,
        args.relay_local_key_path,
        args.relay_cert_path,
    )
    .expect("Failed to start libp2p relay server");

    let addr = SocketAddr::new(args.server.http_addr, args.server.http_port);
    let proxy_server = Arc::new(Proxy::new(
        addr,
        if args.server.http_cors_origins.is_empty() {
            None
        } else {
            Some(args.server.http_cors_origins)
        },
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

// Parses clap cli argument which is expected to be in the format:
// - erc_type:address:start_block
// - address:start_block (erc_type defaults to ERC20)
fn parse_erc_contract(part: &str) -> anyhow::Result<Contract> {
    match part.split(':').collect::<Vec<&str>>().as_slice() {
        [r#type, address] => {
            let r#type = r#type.parse::<ContractType>()?;
            if r#type == ContractType::WORLD {
                return Err(anyhow::anyhow!(
                    "World address cannot be specified as an ERC contract"
                ));
            }

            let address = Felt::from_str(address)
                .with_context(|| format!("Expected address, found {}", address))?;
            Ok(Contract { address, r#type })
        }
        _ => Err(anyhow::anyhow!("Invalid contract format")),
    }
}
