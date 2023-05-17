use clap::Parser;
use futures::join;
use graphql::server::start_graphql;
use sqlx::sqlite::SqlitePoolOptions;
// #[cfg(feature = "postgres")]
// use sqlx::postgres::{PgPoolOptions};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::fmt;

// use crate::indexer::start_indexer;

// mod processors;

mod graphql;
// mod indexer;
mod tests;

/// Dojo World Indexer
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The world to index
    #[arg(short, long)]
    world: String,
    /// The rpc endpoint to use
    #[arg(long)]
    rpc: String,
    /// The Apibara node to use
    #[arg(short, long)]
    apibara: Option<String>,
    /// Database url
    #[arg(short, long, default_value = "sqlite::memory:")]
    database_url: String,
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
        let cts = cts.clone();
        move || {
            cts.cancel();
        }
    })?;

    // let world = BigUint::from_str_radix(&args.world[2..], 16).unwrap_or_else(|error| {
    //     panic!("Failed parsing world address: {error:?}");
    // });

    let database_url = &args.database_url;
    #[cfg(feature = "sqlite")]
    let pool = SqlitePoolOptions::new().max_connections(5).connect(database_url).await?;
    // #[cfg(feature = "postgres")]
    // let pool = PgPoolOptions::new().max_connections(5).connect(database_url).await?;

    // let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(&args.rpc).unwrap()));

    let graphql = start_graphql(&pool);
    // let indexer = start_indexer(cts.clone(), world, node_uri, &pool, &provider);

    let _ = join!(graphql); //, indexer);

    Ok(())
}
