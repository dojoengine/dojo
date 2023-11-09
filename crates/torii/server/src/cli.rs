mod proxy;

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use dojo_world::contracts::world::WorldContractReader;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::signal::ctrl_c;
#[cfg(target_family = "unix")]
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tokio_stream::StreamExt;
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::metadata_update::MetadataUpdateProcessor;
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::processors::store_transaction::StoreTransactionProcessor;
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::Sql;
use torii_core::types::Model;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};
use url::Url;

use crate::proxy::Proxy;

/// Dojo World Indexer
#[derive(Parser, Debug)]
#[command(name = "torii", author, version, about, long_about = None)]
struct Args {
    /// The world to index
    #[arg(short, long = "world", env = "DOJO_WORLD_ADDRESS")]
    world_address: FieldElement,
    /// The rpc endpoint to use
    #[arg(long, default_value = "http://localhost:5050")]
    rpc: String,
    /// Database filepath (ex: indexer.db). If specified file doesn't exist, it will be
    /// created. Defaults to in-memory database
    #[arg(short, long, default_value = ":memory:")]
    database: String,
    /// Specify a block to start indexing from, ignored if stored head exists
    #[arg(short, long, default_value = "0")]
    start_block: u64,
    /// Host address for api endpoints
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    /// Port number for api endpoints
    #[arg(long, default_value = "8080")]
    port: u16,
    /// Specify allowed origins for api endpoints (comma-separated list of allowed origins, or "*"
    /// for all)
    #[arg(long, default_value = "*")]
    #[arg(value_delimiter = ',')]
    allowed_origins: Vec<String>,
    /// The external url of the server, used for configuring the GraphQL Playground in a hosted
    /// environment
    #[arg(long)]
    external_url: Option<Url>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hyper_reverse_proxy=off"));

    let subscriber = fmt::Subscriber::builder().with_env_filter(filter_layer).finish();

    // Set the global subscriber
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set the global tracing subscriber");

    // Setup cancellation for graceful shutdown
    let (shutdown_tx, _) = broadcast::channel(1);

    let sigint = ctrl_c();

    #[cfg(target_family = "unix")]
    let mut sigterm = signal(SignalKind::terminate())?;
    #[cfg(target_family = "windows")]
    let (_, sigterm) = futures::channel::mpsc::unbounded::<()>();

    let database_url = format!("sqlite:{}", &args.database);
    let options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .min_connections(1)
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("../migrations").run(&pool).await?;

    let provider: Arc<_> = JsonRpcClient::new(HttpTransport::new(Url::parse(&args.rpc)?)).into();

    // Get world address
    let world = WorldContractReader::new(args.world_address, &provider);

    let mut db = Sql::new(pool.clone(), args.world_address).await?;
    let processors = Processors {
        event: vec![
            Box::new(RegisterModelProcessor),
            Box::new(StoreSetRecordProcessor),
            Box::new(MetadataUpdateProcessor),
        ],
        transaction: vec![Box::new(StoreTransactionProcessor)],
        ..Processors::default()
    };

    let (block_tx, block_rx) = tokio::sync::mpsc::channel(100);

    let mut engine = Engine::new(
        world,
        &mut db,
        &provider,
        processors,
        EngineConfig { start_block: args.start_block, ..Default::default() },
        shutdown_tx.clone(),
        Some(block_tx),
    );

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    let shutdown_rx = shutdown_tx.subscribe();
    let (grpc_addr, grpc_server) = torii_grpc::server::new(
        shutdown_rx,
        &pool,
        block_rx,
        args.world_address,
        Arc::clone(&provider),
    )
    .await?;

    let proxy_server = Arc::new(Proxy::new(addr, args.allowed_origins, Some(grpc_addr), None));

    let graphql_server = spawn_rebuilding_graphql_server(
        shutdown_tx.clone(),
        pool.into(),
        args.external_url,
        proxy_server.clone(),
    );

    info!("🚀 Torii listening at {}", format!("http://{}", addr));
    info!("Graphql playground: {}\n", format!("http://{}/graphql", addr));

    tokio::select! {
        _ = sigterm.recv() => {
            let _ = shutdown_tx.send(());
        }
        _ = sigint => {
            let _ = shutdown_tx.send(());
        }

        _ = engine.start() => {},
        _ = proxy_server.start(shutdown_tx.subscribe()) => {},
        _ = graphql_server => {},
        _ = grpc_server => {},
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
