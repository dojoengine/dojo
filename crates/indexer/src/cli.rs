use clap::Parser;
use futures::StreamExt;
use std::{cmp::Ordering, error::Error, vec};
mod stream;
use apibara_client_protos::pb::starknet::v1alpha2::{Filter, HeaderFilter};
use log::{debug, info, warn};
use prisma_client_rust::bigdecimal::{
    num_bigint::{BigUint},
    Num,
};
mod prisma;

/// Command line args parser.
/// Exits with 0/1 if the input is formatted correctly/incorrectly.
#[derive(Parser, Debug)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// The world to index
    world: String,
    /// The RPC endpoint to use
    rpc: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let world = BigUint::from_str_radix(&args.world[2..], 16).unwrap_or_else(|error| {
        panic!("Failed parsing world address: {error:?}");
    });
    let rpc = &args.rpc;

    let client = prisma::PrismaClient::_builder().build().await;
    assert!(client.is_ok());

    let stream = stream::ApibaraClient::new(rpc).await;
    match stream {
        std::result::Result::Ok(s) => {
            println!("Connected");
            start(s, client.unwrap(), world).await.unwrap_or_else(|error| {
                panic!("Failed starting: {error:?}");
            });
        }
        std::result::Result::Err(e) => panic!("Error: {:?}", e),
    }

    Ok(())
}

async fn start(
    mut stream: stream::ApibaraClient,
    _client: prisma::PrismaClient,
    world: BigUint,
) -> Result<(), Box<dyn Error>> {
    let data_stream = stream
        .request_data({
            Filter {
                header: Some(HeaderFilter { weak: true }),
                transactions: vec![],
                events: vec![],
                messages: vec![],
                state_update: None,
            }
        })
        .await?;
    futures::pin_mut!(data_stream);

    let mut world_deployed = false;

    while let Some(mess) = data_stream.next().await {
        match mess {
            Ok(Some(mess)) => {
                debug!("Received message");
                let data = &mess.data;
                // TODO: pending data
                //let end_cursor = &data.end_cursor;
                //let cursor = &data.cursor;
                for block in data {
                    match &block.header {
                        Some(header) => {
                            info!("Received block {}", header.block_number);
                        }
                        None => {
                            warn!("Received block without header");
                        }
                    }

                    // wait for our world contract to be deployed
                    if !world_deployed {
                        match &block.state_update {
                            Some(state_update) => match &state_update.state_diff {
                                Some(state_diff) => {
                                    for contract in &state_diff.deployed_contracts {
                                        if Ordering::is_eq(
                                            contract
                                                .contract_address
                                                .as_ref()
                                                .unwrap()
                                                .to_biguint()
                                                .cmp(&world),
                                        ) {
                                            world_deployed = true;
                                            break;
                                        }
                                    }
                                }
                                None => (),
                            },
                            None => (),
                        }

                        if !world_deployed {
                            continue;
                        }
                    }

                    for event in &block.events {
                        let _tx_hash = &event
                            .transaction
                            .as_ref()
                            .unwrap()
                            .meta
                            .as_ref()
                            .unwrap()
                            .hash
                            .as_ref()
                            .unwrap()
                            .to_biguint();
                        match &event.event {
                            Some(_ev_data) => {
                                // handle event
                            }
                            None => {
                                warn!("Received event without key");
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                continue;
            }
            Err(e) => {
                warn!("Error: {:?}", e);
            }
        }
    }

    Ok(())
}
