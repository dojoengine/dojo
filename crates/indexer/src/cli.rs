use std::{error::Error, vec};
use anyhow::{Context};
use clap::Parser;
use hex_literal::hex;
use futures::StreamExt;
mod client;
use tokio::sync::mpsc;
use log::{info, debug, warn};
use apibara_client_protos::pb::starknet::v1alpha2::{Filter, HeaderFilter, StateUpdateFilter};


// Won't compile until apibara merges the fix for their version of anyhow

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

    let mut world = [0u8; 32];
    assert_eq!(hex::decode_to_slice(args.world, &mut world), Ok(()));

    let rpc = &args.rpc;

    let stream = client::ApibaraClient::new(rpc).await;
    match stream {
        std::result::Result::Ok(s) => {
            println!("Connected");
            start(s, world).await;
        },
        std::result::Result::Err(e) => println!("Error: {:?}", e),
    }


    Ok(())
}

async fn start(mut stream: client::ApibaraClient, world: [u8; 32]) -> Result<(), Box<dyn Error>> {
    let mut data_stream = stream.request_data({Filter { header: Some(HeaderFilter{weak:true}), transactions: vec![], events: vec![], messages: vec![], state_update: None }}).await?;
    futures::pin_mut!(data_stream);

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
                        },
                        None => {
                            warn!("Received block without header");
                        }
                    }
                    for event in &block.events {
                        let tx_hash = &event.transaction.as_ref().unwrap().meta.as_ref().unwrap().hash.as_ref().unwrap().to_biguint();
                        match &event.event {
                            Some(ev_data) => {
                                // handle event
                            },
                            None => {
                                warn!("Received event without key");
                            }
                        }
                    }
                }
            },
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
