use std::cmp::Ordering;
use std::error::Error;
use std::vec;

use clap::Parser;
use futures::StreamExt;
mod stream;
use apibara_client_protos::pb::starknet::v1alpha2::{
    DeployedContractFilter, EventFilter, FieldElement, Filter, HeaderFilter, StateUpdateFilter,
};
use log::{debug, info, warn};
use prisma_client_rust::bigdecimal::num_bigint::BigUint;
use prisma_client_rust::bigdecimal::Num;
use processors::{BlockProcessor, TransactionProcessor};

use crate::hash::starknet_hash;
use crate::processors::component_register::ComponentRegistrationProcessor;
use crate::processors::component_state_update::ComponentStateUpdateProcessor;
use crate::processors::system_register::SystemRegistrationProcessor;
use crate::processors::EventProcessor;
mod processors;

#[allow(warnings, unused, elided_lifetimes_in_paths)]
mod prisma;

mod hash;
mod server;

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

struct Processors {
    event_processors: Vec<Box<dyn EventProcessor>>,
    block_processors: Vec<Box<dyn BlockProcessor>>,
    transaction_processors: Vec<Box<dyn TransactionProcessor>>,
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
            start(s, client.unwrap(), &processors, world).await.unwrap_or_else(|error| {
                panic!("Failed starting: {error:?}");
            });
        }
        std::result::Result::Err(e) => panic!("Error: {:?}", e),
    }

    Ok(())
}

fn filter_by_processors(filter: &mut Filter, processors: &Processors) {
    for processor in &processors.event_processors {
        let bytes: [u8; 32] =
            starknet_hash(processor.get_event_key().as_bytes()).to_bytes_be().try_into().unwrap();

        filter.events.push(EventFilter {
            keys: vec![FieldElement::from_bytes(&bytes)],
            ..Default::default()
        })
    }
}

async fn start(
    mut stream: stream::ApibaraClient,
    client: prisma::PrismaClient,
    processors: &Processors,
    world: BigUint,
) -> Result<(), Box<dyn Error>> {
    let mut filter = Filter {
        header: Some(HeaderFilter { weak: true }),
        transactions: vec![],
        events: vec![],
        messages: vec![],
        state_update: Some(StateUpdateFilter {
            deployed_contracts: vec![DeployedContractFilter {
                // we just want to know when our world contract is deployed
                contract_address: Some(FieldElement::from_bytes(
                    &world.to_bytes_be().try_into().unwrap(),
                )),
                ..Default::default()
            }],
            ..Default::default()
        }),
    };

    // filter requested data by the events we process
    filter_by_processors(&mut filter, processors);

    let data_stream = stream
        .request_data({
            Filter {
                header: Some(HeaderFilter { weak: false }),
                transactions: vec![],
                events: vec![],
                messages: vec![],
                state_update: None,
            }
        })
        .await?;
    futures::pin_mut!(data_stream);

    // dont process anything until our world is deployed
    let mut world_deployed = false;
    while let Some(mess) = data_stream.next().await {
        match mess {
            Ok(Some(mess)) => {
                debug!("Received message");
                let data = &mess.data;

                for block in data {
                    match &block.header {
                        Some(header) => {
                            info!("Received block {}", header.block_number);

                            for processor in &processors.block_processors {
                                processor.process(&client, block.clone()).await.unwrap_or_else(
                                    |op| {
                                        panic!("Failed processing block: {op:?}");
                                    },
                                )
                            }
                        }
                        None => {
                            warn!("Received block without header");
                        }
                    }

                    // wait for our world contract to be deployed
                    if !world_deployed {
                        let state = block.state_update.as_ref();
                        if state.is_some() && state.unwrap().state_diff.is_some() {
                            let state_diff = state.unwrap().state_diff.as_ref().unwrap();
                            for contract in state_diff.deployed_contracts.iter() {
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

                        if !world_deployed {
                            continue;
                        }
                    }

                    for transaction in &block.transactions {
                        match &transaction.receipt {
                            Some(_tx) => {
                                for processor in &processors.transaction_processors {
                                    processor
                                        .process(&client, transaction.clone())
                                        .await
                                        .unwrap_or_else(|op| {
                                            panic!("Failed processing transaction: {op:?}");
                                        })
                                }
                            }
                            None => {}
                        }
                    }

                    for event in &block.events {
                        match &event.event {
                            Some(_ev_data) => {
                                for processor in &processors.event_processors {
                                    processor.process(&client, event.clone()).await.unwrap_or_else(
                                        |op| {
                                            panic!("Failed processing event: {op:?}");
                                        },
                                    );
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
