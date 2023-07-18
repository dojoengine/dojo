use camino::Utf8PathBuf;
use clap::Parser;
use dojo_world::manifest::Manifest;
use graphql::server::start_graphql;
use sqlx::sqlite::SqlitePoolOptions;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use state::sql::Sql;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing_subscriber::fmt;
use url::Url;

use crate::engine::Processors;
use crate::indexer::Indexer;
use crate::processors::register_component::RegisterComponentProcessor;
use crate::processors::register_system::RegisterSystemProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::state::State;

mod engine;
mod graphql;
mod indexer;
mod processors;
mod state;
mod types;

#[cfg(test)]
mod tests;

/// Dojo World Indexer
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The world to index
    #[arg(short, long, default_value = "0x420")]
    world_address: FieldElement,
    /// The rpc endpoint to use
    #[arg(long, default_value = "http://localhost:5050")]
    rpc: String,
    /// Database url
    #[arg(short, long, default_value = "sqlite::memory:")]
    database_url: String,
    /// Specify a local manifest to intiailize from
    #[arg(short, long)]
    manifest: Option<Utf8PathBuf>,
    /// Specify a block to start indexing from, ignored if stored head exists
    #[arg(short, long)]
    start_block: Option<u64>,
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
    sqlx::migrate!().run(&pool).await?;

    let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(&args.rpc).unwrap()));

    let manifest = if let Some(manifest_path) = args.manifest {
        Manifest::load_from_path(manifest_path).expect("Failed to load manifest")
    } else {
        Manifest::default()
    };

    let state = Sql::new(pool.clone(), args.world_address).await?;
    state.load_from_manifest(manifest.clone()).await?;
    let processors = Processors {
        event: vec![
            Box::new(RegisterComponentProcessor),
            Box::new(RegisterSystemProcessor),
            Box::new(StoreSetRecordProcessor),
        ],
        ..Processors::default()
    };

    let indexer = Indexer::new(&state, &provider, processors, manifest, args.start_block);
    let graphql = start_graphql(&pool);

    tokio::select! {
        res = indexer.start() => {
            if let Err(e) = res {
                error!("Indexer failed with error: {:?}", e);
            }
        }
        res = graphql => {
            if let Err(e) = res {
                error!("GraphQL server failed with error: {:?}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down");
        }
    }

    Ok(())
}
