use std::error::Error;
use anyhow::{Context, Ok};
use clap::Parser;
use hex_literal::hex;
use client::ApibaraClient;
use futures::StreamExt;

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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let world: [u8; 32] = hex!(args.world);
    let rpc = &args.rpc;

    let stream = client::ApibaraClient::new(rpc).await;
    match stream {
        Ok(s) => {
            println!("Connected");
            start(s, world).await;
        },
        Err(e) => println!("Error: {:?}", e),
    }


    Ok(())
}

async fn start(mut stream: ApibaraClient, world: [u8; 32]) -> Result<(), Box<dyn Error>> {
    let connection = sqlite::open(":memory:").unwrap();

    let mut data_stream = stream.request_data(client::Filter { world }.into()).await?;
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
