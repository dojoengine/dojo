#[cfg(test)]
mod test {
    use std::error::Error;
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::Duration;

    use futures::StreamExt;
    #[cfg(not(target_arch = "wasm32"))]
    use tokio::time::sleep;
    #[cfg(not(target_arch = "wasm32"))]
    use tokio::{self, select};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_futures::spawn_local;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    use crate::client::{Libp2pClient, Message};
    #[cfg(not(target_arch = "wasm32"))]
    use crate::server::Libp2pRelay;
    use crate::types::ClientMessage;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    // This tests subscribing to a topic and receiving a message
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_client_messaging() -> Result<(), Box<dyn Error>> {
        let _ = tracing_subscriber::fmt().with_env_filter("torii_libp2p=debug").try_init();
        // Initialize the relay server
        let mut relay_server: Libp2pRelay = Libp2pRelay::new(1010, 2020)?;

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

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn test_client_connection_wasm() -> Result<(), Box<dyn Error>> {
        use tracing::info;

        tracing_wasm::set_as_global_default();

        let _ = tracing_subscriber::fmt().with_env_filter("torii_libp2p=debug").try_init();
        // Initialize the first client (listener)
        let mut client = Libp2pClient::new(
            "/ip4/127.0.0.1/udp/2020/webrtc-direct/certhash/\
             uEiCKqki8Ie3cU0IKWE207wmPDBeWC5H1G7kMYrUpqaPmWw"
                .to_string(),
        )?;
        client.subscribe("mawmaw")?;
        let (sender, mut receiver) = futures::channel::mpsc::unbounded::<Message>();

        spawn_local(async move {
            // let res = client.publish(&ClientMessage {
            //     topic: "mawmaw".to_string(),
            //     data: "324523".as_bytes().to_vec(),
            // });
            // info!("Result: {:?}", res);
            client.run(&sender).await;
        });

        Ok(())

        // let timeout = wasm_timer::Delay::new(std::time::Duration::from_secs(5));
        // let message_future = receiver.next();

        // match futures::future::select(Box::pin(message_future), Box::pin(timeout)).await {
        //     Either::Left((Some(_message), _)) => {
        //         println!("Test OK: Received message within 5 seconds.");
        //         Ok(())
        //     }
        //     _ => {
        //         println!("Test Failed: Did not receive message within 5 seconds.");
        //         Err("Timeout reached without receiving a message".into())
        //     }
        // }
    }
}
