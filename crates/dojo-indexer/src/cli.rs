use std::str::FromStr;
use std::vec;

use apibara_sdk::Uri;
use clap::Parser;
use futures::join;
use num::{BigUint, Num};
use sqlx::sqlite::SqlitePoolOptions;
// #[cfg(feature = "postgres")]
// use sqlx::postgres::{PgPoolOptions};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use url::Url;

use crate::indexer::{start_indexer, Processors};
use crate::processors::component_register::ComponentRegistrationProcessor;
use crate::processors::component_state_update::ComponentStateUpdateProcessor;
use crate::processors::system_register::SystemRegistrationProcessor;
use crate::server::start_server;
use crate::stream::StarknetClientBuilder;

mod processors;

mod graphql;
mod hash;
mod indexer;
mod server;
mod stream;

/// Command line args parser.
/// Exits with 0/1 if the input is formatted correctly/incorrectly.
#[derive(Parser, Debug)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// The world to index
    world: String,
    /// The Apibara node to use
    node: String,
    /// The rpc endpoint to use
    rpc: String,
    /// Database url
    database_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let world = BigUint::from_str_radix(&args.world[2..], 16).unwrap_or_else(|error| {
        panic!("Failed parsing world address: {error:?}");
    });

    let database_url = &args.database_url;
    #[cfg(feature = "sqlite")]
    let pool = SqlitePoolOptions::new().max_connections(5).connect(database_url).await?;
    // #[cfg(feature = "postgres")]
    // let pool = PgPoolOptions::new().max_connections(5).connect(database_url).await?;
    let node = Uri::from_str(&args.node)?;
    let stream = StarknetClientBuilder::default().connect(node).await;

    let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(&args.rpc).unwrap()));

    let processors = Processors {
        event_processors: vec![
            Box::new(ComponentStateUpdateProcessor::new()),
            Box::new(ComponentRegistrationProcessor::new()),
            Box::new(SystemRegistrationProcessor::new()),
        ],
        block_processors: vec![],
        transaction_processors: vec![],
    };

    match stream {
        std::result::Result::Ok((data_stream, stream_client)) => {
            println!("Connected");
            let graphql = start_server(&pool);
            let indexer =
                start_indexer(data_stream, stream_client, &pool, &provider, &processors, world);
            let _res = join!(graphql, indexer);
        }
        std::result::Result::Err(e) => panic!("Error: {e:?}"),
    }

    Ok(())
}
