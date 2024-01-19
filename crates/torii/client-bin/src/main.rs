use std::error::Error;
use futures::StreamExt;
use libp2p::gossipsub;
use tokio::{io, io::AsyncBufReadExt, select};
use torii_client::client::Client;
use torii_libp2p::types::ServerMessage;
use torii_libp2p::{
    client::{events::ClientEvent, Libp2pClient},
    types::ClientMessage,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let relay_server_addr = "/ip6/::1/tcp/1010".parse()?;

    let client = Client::new(
        "http://127.0.0.1:8080".to_string(),
        "http://127.0.0.1:5050".to_string(),
        relay_server_addr,
        "0x28f5999ae62fec17c09c52a800e244961dba05251f5aaf923afabd9c9804d1a".parse()?,
        Some(vec![]),
    )
    .await?;

    let x = client.subscribe_topic("mawmaw")?;
    println!("Subscribed to topic: {:?}", x);

    let (sender, mut receiver) = futures::channel::mpsc::unbounded::<ServerMessage>();
    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    loop {
        select! {
            _ = client.listen_messages(&sender) => {},
            Ok(Some(line)) = stdin.next_line() => {
                client.publish_message("mawmaw", line.as_bytes())?;
            }
            Some(event) = receiver.next() => {
                println!("Received message: {:?}", event);
            }
        }
    }
}