use std::error::Error;
use anyhow::{Context, Ok};
use clap::Parser;
use hex_literal::hex;
use client::ApibaraClient;
use futures::StreamExt;




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
                                let address = ev_data.from_address.as_ref().unwrap().to_hex_string();
                                let data: Vec<BigUint> = ev_data.data.iter().map(|item| item.to_biguint()).collect();
                                let from = data[0].to_str_radix(16);
                                let to = data[1].to_str_radix(16);
                                let token_id = (&data[2] + &data[3] * 2.to_biguint().unwrap().pow(128)).to_str_radix(16);
                                info!("Received event from 0x{} to 0x{} token 0x{} in TX 0x{}", from, to, token_id, tx_hash.to_str_radix(16));
                                let mut statement = connection.prepare(query).unwrap();
                                statement.bind::<&[(_, Value)]>(&[
                                    (1, from.into()),
                                    (2, to.into()),
                                    (3, token_id.into()),
                                    (4, block.header.as_ref().unwrap().block_number.to_i64().unwrap_or(0).into()),
                                    (5, sqlite::Value::Null)
                                ])?;
                                statement.next();
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
