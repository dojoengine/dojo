use std::{error::Error};

use tokio::{io, io::AsyncBufReadExt, select};
use torii_libp2p::client::{RelayClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let relay_server_addr = "/ip6/::1/tcp/1010".parse()?;
    let mut client = RelayClient::new(relay_server_addr)?;

    // subscribe to topic
    client.subscribe("mimi")?;

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    // select that client is running and we have a line from stdin
    loop {
        select! {
            _ = client.run() => {},
            Ok(Some(line)) = stdin.next_line() => {
                client.publish("mimi", &line)?;
            }
        }
    }
}