use std::cmp::Ordering;
use std::error::Error;

use apibara_core::node::v1alpha2::DataFinality;
use apibara_core::starknet::v1alpha2::{
    DeployedContractFilter, EventFilter, FieldElement, Filter, HeaderFilter, StateUpdateFilter,
};
use apibara_sdk::{Configuration, DataMessage, Uri};
use futures::TryStreamExt;
use log::{debug, info, warn};
use num::BigUint;
use sqlx::{Pool, Sqlite};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};

use crate::hash::starknet_hash;
use crate::processors::component_register::ComponentRegistrationProcessor;
use crate::processors::component_state_update::ComponentStateUpdateProcessor;
use crate::processors::system_register::SystemRegistrationProcessor;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::stream::{FieldElementExt, StarknetClientBuilder};

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
    world: BigUint,
    node_uri: Uri,
    pool: &Pool<Sqlite>,
    provider: &JsonRpcClient<HttpTransport>,
) -> Result<(), Box<dyn Error>> {
    let processors = Processors {
        event_processors: vec![
            Box::new(ComponentStateUpdateProcessor::new()),
            Box::new(ComponentRegistrationProcessor::new()),
            Box::new(SystemRegistrationProcessor::new()),
        ],
        block_processors: vec![],
        transaction_processors: vec![],
    };

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

    let (mut data_stream, stream_client) =
        StarknetClientBuilder::default().connect(node_uri).await?;

    // filter requested data by the events we process
    filter_by_processors(&mut filter, &processors);

    // TODO: should set starting block.
    let starting_configuration = Configuration::<Filter>::default()
        .with_finality(DataFinality::DataStatusAccepted)
        .with_filter(|filter| filter.with_header(HeaderFilter { weak: false }));
    stream_client.send(starting_configuration).await?;

    // dont process anything until our world is deployed
    let mut world_deployed = false;
    while let Some(message) = data_stream.try_next().await? {
        debug!("Received message");
        match message {
            DataMessage::Invalidate { cursor } => {
                panic!("chain reorganization: {cursor:?}");
            }
            DataMessage::Data { batch, .. } => {
                for block in batch {
                    match &block.header {
                        Some(header) => {
                            info!("Received block {}", header.block_number);

                            for processor in &processors.block_processors {
                                processor
                                    .process(pool, provider, block.clone())
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
                                        .process(pool, provider, transaction.clone())
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
                                        .process(pool, provider, event.clone())
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
        }
    }

    Ok(())
}
