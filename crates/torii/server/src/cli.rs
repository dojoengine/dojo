mod server;

use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use server::Server;
use sqlx::sqlite::SqlitePoolOptions;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio_util::sync::CancellationToken;
use torii_client::contract::world::WorldContractReader;
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::metadata_update::MetadataUpdateProcessor;
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::sql::Sql;
use tracing::error;
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
    /// Database url
    #[arg(short, long, default_value = "sqlite::memory:")]
    database_url: String,
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
    let cts = CancellationToken::new();
    ctrlc::set_handler({
        let cts: CancellationToken = cts.clone();
        move || {
            cts.cancel();
        }
    })?;

    let database_url = &args.database_url;
    #[cfg(feature = "sqlite")]
    let pool = SqlitePoolOptions::new().max_connections(5).connect(database_url).await?;
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
        // transaction: vec![Box::new(StoreSystemCallProcessor)],
        ..Processors::default()
    };

    let (block_sender, block_receiver) = tokio::sync::mpsc::channel(100);

    let mut engine = Engine::new(
        &world,
        &mut db,
        &provider,
        processors,
        EngineConfig { start_block: args.start_block, ..Default::default() },
        Some(block_sender),
    );

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    let server = Server::new(
        addr,
        pool,
        block_receiver,
        args.world_address,
        Arc::clone(&provider),
        args.allowed_origins,
    );

    tokio::select! {
        res = engine.start(cts) => {
            if let Err(e) = res {
                error!("Indexer failed with error: {e}");
            }
        }

        res = server.start() => {
            if let Err(e) = res {
                error!("Server failed with error: {e}");
            }
        }

        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down");
        }
    }

    Ok(())
}
