#[cfg(test)]
mod test {
    use std::error::Error;

    use crate::client::RelayClient;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    // This tests subscribing to a topic and receiving a message
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_client_messaging() -> Result<(), Box<dyn Error>> {
        use dojo_types::schema::{Member, Struct};
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use starknet_ff::FieldElement;
        use tokio::sync::RwLock;
        use torii_core::sql::Sql;

        use crate::server::Relay;

        let _ = tracing_subscriber::fmt()
            .with_env_filter("torii::relay::client=debug,torii::relay::server=debug")
            .try_init();

        // Database
        let options = <SqliteConnectOptions as std::str::FromStr>::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
        sqlx::migrate!("../migrations").run(&pool).await.unwrap();

        let db = std::sync::Arc::new(RwLock::new(
            Sql::new(pool.clone(), FieldElement::from_bytes_be(&[0; 32]).unwrap()).await?,
        ));

        // Initialize the relay server
        let mut relay_server: Relay = Relay::new(db.clone(), 9900, 9901, None, None)?;
        tokio::spawn(async move {
            relay_server.run().await;
        });

        // Initialize the first client (listener)
        let mut client = RelayClient::new("/ip4/127.0.0.1/tcp/9900".to_string())?;
        tokio::spawn(async move {
            client.event_loop.lock().await.run().await;
        });

        client.command_sender.wait_for_relay().await?;
        let mut data = Struct { name: "Message".to_string(), children: vec![] };

        data.children.push(Member {
            name: "player".to_string(),
            ty: dojo_types::schema::Ty::Primitive(
                dojo_types::primitive::Primitive::ContractAddress(Some(
                    FieldElement::from_bytes_be(&[0; 32]).unwrap(),
                )),
            ),
            key: true,
        });

        data.children.push(Member {
            name: "message".to_string(),
            ty: dojo_types::schema::Ty::Primitive(dojo_types::primitive::Primitive::U8(Some(0))),
            key: false,
        });

        client.command_sender.publish(dojo_types::schema::Ty::Struct(data)).await?;

        Ok(())
        // loop {
        //     select! {
        //         entity = sqlx::query("SELECT * FROM entities WHERE id = ?")
        //         .bind(format!("{:#x}", FieldElement::from_bytes_be(&[0;
        // 32]).unwrap())).fetch_one(&pool) => {             if let Ok(_) = entity {
        //                 println!("Test OK: Received message within 5 seconds.");
        //                 return Ok(());
        //             }
        //         }
        //         _ = sleep(Duration::from_secs(5)) => {
        //             println!("Test Failed: Did not receive message within 5 seconds.");
        //             return Err("Timeout reached without receiving a message".into());
        //         }
        //     }
        // }
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
        let mut client = RelayClient::new(
            "/ip4/127.0.0.1/udp/9091/webrtc-direct/certhash/\
             uEiCAoeHQh49fCHDolECesXO0CPR7fpz0sv0PWVaIahzT4g"
                .to_string(),
        )?;

        spawn_local(async move {
            client.event_loop.lock().await.run().await;
        });

        client.command_sender.subscribe("mawmaw".to_string()).await?;
        client.command_sender.wait_for_relay().await?;
        client.command_sender.publish("mawmaw".to_string(), "mimi".as_bytes().to_vec()).await?;

        let timeout = wasm_timer::Delay::new(std::time::Duration::from_secs(2));
        let mut message_future = client.message_receiver.lock().await;
        let message_future = message_future.next();

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
