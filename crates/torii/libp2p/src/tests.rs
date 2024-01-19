#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        client::Libp2pClient,
        server::Libp2pRelay,
        types::{ClientMessage, ServerMessage},
    };
    use futures::StreamExt;
    use std::{error::Error, time::Duration};
    use tokio::{
        self, select,
        time::{self, sleep},
    };

    // This tests subscribing to a topic and receiving a message
    #[tokio::test]
    async fn test_client_messaging() -> Result<(), Box<dyn Error>> {
        // Initialize the relay server
        let relay_server = Libp2pRelay::new(Some(true), 1010)?;

        // Give some time for the server to start up
        sleep(Duration::from_secs(1)).await;

        // Initialize the first client (listener)
        let mut client = Libp2pClient::new("/ip6/::1/tcp/1010".to_string())?;
        client.subscribe("mawmaw")?;
        let (sender, mut receiver) = futures::channel::mpsc::unbounded::<ServerMessage>();

        tokio::spawn(async move {
            relay_server.await;
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
