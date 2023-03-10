use std::vec;

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
// use crate::server::start_server;

mod processors;

mod graphql;
mod hash;
mod indexer;
mod server;

mod stream;
// mod schema;
// mod model;

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
    let node = &args.node;

    let database_url = &args.database_url;
    #[cfg(feature = "sqlite")]
    let pool = SqlitePoolOptions::new().max_connections(5).connect(database_url).await?;
    // #[cfg(feature = "postgres")]
    // let pool = PgPoolOptions::new().max_connections(5).connect(database_url).await?;

    let stream = stream::ApibaraClient::new(node).await;

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
        std::result::Result::Ok(s) => {
            println!("Connected");
            let graphql = start_server(&pool);
            let indexer = start_indexer(s, &pool, &provider, &processors, world);
            let _res = join!(graphql, indexer);
        }
        std::result::Result::Err(e) => panic!("Error: {:?}", e),
    }

    Ok(())
}
