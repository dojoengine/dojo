use std::error::Error;

use futures::pin_mut;
use futures::StreamExt;
use libp2p::gossipsub;
use tokio::{io, io::AsyncBufReadExt, select};
use torii_libp2p::{
    client::{events::ClientEvent, Libp2pClient},
    types::ClientMessage,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let relay_server_addr = "/ip6/::1/tcp/1010".parse()?;
    let mut client = Libp2pClient::new(relay_server_addr)?;

    // subscribe to topic
    client.subscribe("mimi")?;

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    // select that client is running and we have a line from stdin
    loop {

        select! {
            _ = client.run_message_listener() => {},
            Ok(Some(line)) = stdin.next_line() => {
                client.publish(&ClientMessage{
                    topic: "mimi".to_string(),
                    data: line.as_bytes().to_vec(),
                })?;
            }
            Some(event) = client.receiver.next() => {
                println!("Received message: {:?}", event);
            }
        }
    }
}
