use std::{cmp::Ordering, any::Any};
use std::error::Error;
use std::vec;

use clap::Parser;
use futures::StreamExt;
mod stream;
use apibara_client_protos::pb::starknet::v1alpha2::{Event, Filter, HeaderFilter, EventWithTransaction};
use log::{debug, info, warn};
use prisma_client_rust::bigdecimal::num_bigint::BigUint;
use prisma_client_rust::bigdecimal::Num;
use processors::{IProcessor, component_state_update};
mod processors;

#[allow(warnings, unused, elided_lifetimes_in_paths)]
mod prisma;

mod hash;

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

    let processors: Vec<Box<dyn Any>>  = vec![
        Box::new(component_state_update::ComponentStateUpdateProcessor::new()),
    ];

    match stream {
        std::result::Result::Ok(s) => {
            println!("Connected");
            start(s, client.unwrap(), processors, world).await.unwrap_or_else(|error| {
                panic!("Failed starting: {error:?}");
            });
        }
        std::result::Result::Err(e) => panic!("Error: {:?}", e),
    }

    Ok(())
}

async fn start(
    mut stream: stream::ApibaraClient,
    client: prisma::PrismaClient,
    processors: Vec<Box<dyn Any>>,
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
                // let end_cursor = &data.end_cursor;
                // let cursor = &data.cursor;
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
                                // TODO: only execute event processors
                                for processor in &processors {
                                    if let Some(p) = processor.downcast_ref::<Box<dyn IProcessor<EventWithTransaction>>>() {
                                        p.process(&client, event.clone());
                                    }
                                }
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
