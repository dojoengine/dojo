use std::cmp::Ordering;
use std::error::Error;

use num::BigUint;
use sqlx::{Pool, Sqlite};
use starknet::core::types::FieldElement;
use starknet::core::utils::starknet_keccak;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::processors::component_register::ComponentRegistrationProcessor;
use crate::processors::component_state_update::ComponentStateUpdateProcessor;
use crate::processors::system_register::SystemRegistrationProcessor;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
pub struct Processors<T> {
    pub event_processors: Vec<Box<dyn EventProcessor<T>>>,
    pub block_processors: Vec<Box<dyn BlockProcessor<T>>>,
    pub transaction_processors: Vec<Box<dyn TransactionProcessor<T>>>,
}

fn filter_by_processors<T: JsonRpcTransport>(filter: &mut Filter, processors: &Processors<T>) {
    for processor in &processors.event_processors {
        let bytes: [u8; 32] = starknet_keccak(processor.event_key().as_bytes()).to_bytes_be();

        filter.events.push(EventFilter {
            keys: vec![ApibaraFieldElement::from_bytes(&bytes)],
            ..Default::default()
        })
    }
}

pub async fn start_indexer<T: JsonRpcTransport + Sync + Send>(
    ct: CancellationToken,
    world: BigUint,
    node_uri: Uri,
    pool: &Pool<Sqlite>,
    provider: &JsonRpcClient<T>,
) -> Result<(), Box<dyn Error>> {
    info!("starting indexer");

    let processors = Processors {
        event_processors: vec![
            Box::<ComponentStateUpdateProcessor>::default(),
            Box::<ComponentRegistrationProcessor>::default(),
            Box::<SystemRegistrationProcessor>::default(),
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
                contract_address: Some(ApibaraFieldElement::from_bytes(
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

    // Reduce event processors into a Map that maps event_key to the processor
    let event_processors = processors
        .event_processors
        .into_iter()
        .map(|processor| (starknet_keccak(processor.event_key().as_bytes()), processor))
        .collect::<std::collections::HashMap<_, _>>();

    // dont process anything until our world is deployed
    let mut world_deployed = false;
    while let Some(message) = data_stream.try_next().await? {
        if ct.is_cancelled() {
            return Ok(());
        }

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

                    for event_w_tx in block.events {
                        let event = event_w_tx.clone().event.unwrap();
                        let event_key = event.keys[0].to_biguint();
                        if let Some(processor) = event_processors.get(
                            &FieldElement::from_byte_slice_be(&event_key.to_bytes_be()).unwrap(),
                        ) {
                            processor.process(pool, provider, event_w_tx).await.unwrap_or_else(
                                |op| {
                                    panic!("Failed processing event: {op:?}");
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// #[test]
// fn test_indexer() {
//     use crate::start_apibara;

//     let rpc_url = "http://localhost:5050";
//     let (sequencer, rpc) = build_mock_rpc(5050);
//     let ct = CancellationToken::new();
//     let pool = sqlx::sqlite::SqlitePool::connect("sqlite::memory:").unwrap();
//     let world = BigUint::from(0x1234567890);
//     let provider = JsonRpcClient::new(HttpTransport::new(Uri::parse(rpc_url)));

//     start_apibara(ct, rpc_url.into());
//     start_indexer(ct, world, Uri::from_str("http://localhost:7171").unwrap(), pool, &provider)
// }
