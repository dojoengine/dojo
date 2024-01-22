#[cfg(test)]
mod test {
    use std::error::Error;
    use std::time::Duration;

    use futures::StreamExt;
    use tokio::time::sleep;
    use tokio::{self, select};

    use crate::client::{Libp2pClient, Message};
    use crate::server::Libp2pRelay;
    use crate::types::ClientMessage;

    // This tests subscribing to a topic and receiving a message
    #[tokio::test]
    async fn test_client_messaging() -> Result<(), Box<dyn Error>> {
        // Initialize the relay server
        let mut relay_server: Libp2pRelay = Libp2pRelay::new(1010, 2020)?;
        println!("Relay server initialized");

        // Give some time for the server to start up
        sleep(Duration::from_secs(1)).await;

        // Initialize the first client (listener)
        let mut client = Libp2pClient::new("/ip4/127.0.0.1/tcp/1010".to_string())?;
        client.subscribe("mawmaw")?;
        let (sender, mut receiver) = futures::channel::mpsc::unbounded::<Message>();

        tokio::spawn(async move {
            relay_server.run().await;
        });

        loop {
            select! {
                _ = client.run(&sender) => {},
                event = receiver.next() => {
                    println!("Received message: {:?}", event);
                    return Ok(())
                },
                _ = sleep(Duration::from_secs(2)) => {
                    let res = client.publish(&ClientMessage { topic: "mawmaw".to_string(), data: "324523".as_bytes().to_vec() });
                    println!("Result: {:?}", res);
                },
                _ = sleep(Duration::from_secs(5)) => {
                    return Err("Timed-out waiting for message".into())
                }

            }
        }
    }
}
