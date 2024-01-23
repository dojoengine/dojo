#[cfg(test)]
mod test {
    use crate::client::Libp2pClient;
    use futures::{StreamExt, SinkExt};
    use std::error::Error;
    use crate::types::ClientMessage;
    use crate::client::Command;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    // This tests subscribing to a topic and receiving a message
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_client_messaging() -> Result<(), Box<dyn Error>> {
        use crate::server::Libp2pRelay;
        use std::time::Duration;
        use tokio::time::sleep;
        use tokio::{self, select};


        let _ = tracing_subscriber::fmt().with_env_filter("torii_libp2p=debug").try_init();
        // Initialize the relay server
        let mut relay_server: Libp2pRelay = Libp2pRelay::new(1010, 2020, None, None)?;

        // Give some time for the server to start up
        sleep(Duration::from_secs(1)).await;

        // Initialize the first client (listener)
        let mut client = Libp2pClient::new("/ip4/127.0.0.1/tcp/1010".to_string())?;

        tokio::spawn(async move {
            relay_server.run().await;
        });

        tokio::spawn(async move {
            client.event_loop.run().await;
        });

        // Give some time for the client to start up
        sleep(Duration::from_secs(1)).await;

        client.command_sender.send(Command::Subscribe("mawmaw".to_string())).await?;
        sleep(Duration::from_secs(1)).await;
        client.command_sender.send(Command::Publish(ClientMessage {
            topic: "mawmaw".to_string(),
            data: "324523".as_bytes().to_vec(),
        })).await?;

        loop {
            select! {
                event = client.message_receiver.next() => {
                    if let Some((peer_id, message_id, message)) = event {
                        println!("Received message from {:?} with id {:?}: {:?}", peer_id, message_id, message);
                        return Ok(());
                    }
                }
                _ = sleep(Duration::from_secs(5)) => {
                    println!("Test Failed: Did not receive message within 5 seconds.");
                    return Err("Timeout reached without receiving a message".into());
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_client_connection_wasm() -> Result<(), Box<dyn Error>> {
        use futures::future::{select, Either};
        use wasm_bindgen_futures::spawn_local;

        tracing_wasm::set_as_global_default();

        let _ = tracing_subscriber::fmt().with_env_filter("torii_libp2p=debug").try_init();
        // Initialize the first client (listener)
        // Make sure the cert hash is correct - corresponding to the cert in the relay server
        let mut client = Libp2pClient::new(
            "/ip4/127.0.0.1/udp/9091/webrtc-direct/certhash/\
            uEiDCnWFRzbmT1V8w4b6WI38hwUIUZKy6higxne-3z216eg"
                .to_string(),
        )?;

        spawn_local(async move {
            client.event_loop.run().await;
        });

        // Give some time for the client to start up
        wasm_timer::Delay::new(std::time::Duration::from_secs(1)).await;

        client.command_sender.send(Command::Subscribe("mawmaw".to_string())).await?;
        wasm_timer::Delay::new(std::time::Duration::from_secs(1)).await;
        client.command_sender.send(Command::Publish(ClientMessage {
            topic: "mawmaw".to_string(),
            data: "324523".as_bytes().to_vec(),
        })).await?;

        let timeout = wasm_timer::Delay::new(std::time::Duration::from_secs(5));
        let message_future = client.message_receiver.next();

        match select(message_future, timeout).await {
            Either::Left((Some(_message), _)) => {
                println!("Test OK: Received message within 5 seconds.");
                Ok(())
            }
            _ => {
                println!("Test Failed: Did not receive message within 5 seconds.");
                Err("Timeout reached without receiving a message".into())
            }
        }
    }
}
