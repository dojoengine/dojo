use std::cmp::Ordering;
use std::error::Error;

use apibara_client_protos::pb::starknet::v1alpha2::{
    DeployedContractFilter, EventFilter, FieldElement, Filter, HeaderFilter, StateUpdateFilter,
};
use diesel::Connection;
use futures::StreamExt;
use log::{debug, info, warn};
use prisma_client_rust::bigdecimal::num_bigint::BigUint;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};

use crate::hash::starknet_hash;
use crate::prisma::PrismaClient;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::stream::ApibaraClient;

pub struct Processors {
    pub event_processors: Vec<Box<dyn EventProcessor>>,
    pub block_processors: Vec<Box<dyn BlockProcessor>>,
    pub transaction_processors: Vec<Box<dyn TransactionProcessor>>,
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

pub async fn start_indexer(
    mut stream: ApibaraClient,
    conn: &Connection,
    provider: &JsonRpcClient<HttpTransport>,
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
                                processor
                                    .process(client, provider, block.clone())
                                    .await
                                    .unwrap_or_else(|op| {
                                        panic!("Failed processing block: {op:?}");
                                    })
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
                                        .process(client, provider, transaction.clone())
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
                                    processor
                                        .process(client, provider, event.clone())
                                        .await
                                        .unwrap_or_else(|op| {
                                            panic!("Failed processing event: {op:?}");
                                        });
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
