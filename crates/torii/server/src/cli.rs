mod proxy;

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use dojo_world::contracts::world::WorldContractReader;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::metadata_update::MetadataUpdateProcessor;
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::processors::store_transaction::StoreTransactionProcessor;
use torii_core::sql::Sql;
use tracing_subscriber::fmt;
use url::Url;

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
    let subscriber = fmt::Subscriber::builder()
        .with_max_level(tracing::Level::INFO) // Set the maximum log level
        .finish();

    // Set the global subscriber
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set the global tracing subscriber");

    // Setup cancellation for graceful shutdown
    let (shutdown_tx, _) = broadcast::channel(1);
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

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
    let (graphql_addr, graphql_server) =
        torii_graphql::server::new(shutdown_rx, &pool, args.external_url).await;

    let shutdown_rx = shutdown_tx.subscribe();
    let (grpc_addr, grpc_server) = torii_grpc::server::new(
        shutdown_rx,
        &pool,
        block_rx,
        args.world_address,
        Arc::clone(&provider),
    )
    .await?;

    let shutdown_rx = shutdown_tx.subscribe();
    let proxy_server = proxy::new(shutdown_rx, addr, args.allowed_origins, grpc_addr, graphql_addr);

    tokio::select! {
        _ = sigterm.recv() => {
            let _ = shutdown_tx.send(());
        }
        _ = sigint.recv() => {
            let _ = shutdown_tx.send(());
        }

        _ = engine.start() => {},
        _ = proxy_server.await => {},
        _ = graphql_server => {},
        _ = grpc_server => {},
    };

    Ok(())
}
