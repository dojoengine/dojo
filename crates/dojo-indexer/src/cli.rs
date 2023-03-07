use std::vec;

use clap::Parser;
use futures::join;
use prisma_client_rust::bigdecimal::num_bigint::BigUint;
use prisma_client_rust::bigdecimal::Num;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use url::Url;

use crate::indexer::{start_indexer, Processors};
use crate::processors::component_register::ComponentRegistrationProcessor;
use crate::processors::component_state_update::ComponentStateUpdateProcessor;
use crate::processors::system_register::SystemRegistrationProcessor;
use crate::server::start_server;

mod processors;

mod graphql;
mod hash;
mod indexer;
#[allow(warnings, unused, elided_lifetimes_in_paths)]
mod prisma;
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let world = BigUint::from_str_radix(&args.world[2..], 16).unwrap_or_else(|error| {
        panic!("Failed parsing world address: {error:?}");
    });
    let node = &args.node;

    let client = prisma::PrismaClient::_builder().build().await.unwrap();

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
            let graphql = start_server();
            let indexer = start_indexer(s, &client, &provider, &processors, world);
            let _res = join!(graphql, indexer);
        }
        std::result::Result::Err(e) => panic!("Error: {:?}", e),
    }

    Ok(())
}
