use std::error::Error;

use crate::client::RelayClient;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
use dojo_types::primitive::Primitive;
use katana_runner::KatanaRunner;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

// This tests subscribing to a topic and receiving a message
#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn test_client_messaging() -> Result<(), Box<dyn Error>> {
    use std::sync::Arc;
    use std::time::Duration;

    use dojo_types::schema::{Member, Struct, Ty};
    use dojo_world::contracts::abigen::model::Layout;
    use indexmap::IndexMap;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;
    use starknet::signers::SigningKey;
    use starknet_crypto::Felt;
    use tempfile::NamedTempFile;
    use tokio::select;
    use tokio::sync::broadcast;
    use tokio::time::sleep;
    use torii_core::executor::Executor;
    use torii_core::sql::cache::ModelCache;
    use torii_core::sql::Sql;
    use torii_core::types::{Contract, ContractType};
    use torii_typed_data::typed_data::{Domain, Field, SimpleField, TypedData};

    use crate::server::Relay;
    use crate::types::Message;

    let _ = tracing_subscriber::fmt()
        .with_env_filter("torii::relay::client=debug,torii::relay::server=debug")
        .try_init();

    // Database
    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = <SqliteConnectOptions as std::str::FromStr>::from_str(&path)
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let sequencer = KatanaRunner::new().expect("Failed to create Katana sequencer");

    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let account = sequencer.account_data(0);

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let mut db = Sql::new(
        pool.clone(),
        sender,
        &[Contract { address: Felt::ZERO, r#type: ContractType::WORLD }],
        model_cache,
    )
    .await
    .unwrap();

    // Register the model of our Message
    db.register_model(
        "types_test",
        &Ty::Struct(Struct {
            name: "Message".to_string(),
            children: vec![
                Member {
                    name: "identity".to_string(),
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                    key: true,
                },
                Member {
                    name: "message".to_string(),
                    ty: Ty::ByteArray("".to_string()),
                    key: false,
                },
            ],
        }),
        Layout::Fixed(vec![]),
        Felt::ZERO,
        Felt::ZERO,
        0,
        0,
        0,
        None,
    )
    .await
    .unwrap();
    db.execute().await.unwrap();

    // Initialize the relay server
    let mut relay_server = Relay::new(db, provider, 9900, 9901, 9902, None, None)?;
    tokio::spawn(async move {
        relay_server.run().await;
    });

    // Initialize the first client (listener)
    let client = RelayClient::new("/ip4/127.0.0.1/tcp/9900".to_string())?;
    tokio::spawn(async move {
        client.event_loop.lock().await.run().await;
    });

    let mut typed_data = TypedData::new(
        IndexMap::from_iter(vec![
            (
                "types_test-Message".to_string(),
                vec![
                    Field::SimpleType(SimpleField {
                        name: "identity".to_string(),
                        r#type: "ContractAddress".to_string(),
                    }),
                    Field::SimpleType(SimpleField {
                        name: "message".to_string(),
                        r#type: "string".to_string(),
                    }),
                ],
            ),
            (
                "StarknetDomain".to_string(),
                vec![
                    Field::SimpleType(SimpleField {
                        name: "name".to_string(),
                        r#type: "shortstring".to_string(),
                    }),
                    Field::SimpleType(SimpleField {
                        name: "version".to_string(),
                        r#type: "shortstring".to_string(),
                    }),
                    Field::SimpleType(SimpleField {
                        name: "chainId".to_string(),
                        r#type: "shortstring".to_string(),
                    }),
                    Field::SimpleType(SimpleField {
                        name: "revision".to_string(),
                        r#type: "shortstring".to_string(),
                    }),
                ],
            ),
        ]),
        "types_test-Message",
        Domain::new("types_test-Message", "1", "0x0", Some("1")),
        IndexMap::new(),
    );
    typed_data.message.insert(
        "identity".to_string(),
        torii_typed_data::typed_data::PrimitiveType::String(account.address.to_string()),
    );

    typed_data.message.insert(
        "message".to_string(),
        torii_typed_data::typed_data::PrimitiveType::String("mimi".to_string()),
    );

    let message_hash = typed_data.encode(account.address).unwrap();
    let signature =
        SigningKey::from_secret_scalar(account.private_key.clone().unwrap().secret_scalar())
            .sign(&message_hash)
            .unwrap();

    client
        .command_sender
        .publish(Message { message: typed_data, signature: vec![signature.r, signature.s] })
        .await?;

    sleep(std::time::Duration::from_secs(2)).await;

    loop {
        select! {
            entity = sqlx::query("SELECT * FROM entities").fetch_one(&pool) => if entity.is_ok() {
                println!("Test OK: Received message within 5 seconds.");
                return Ok(());
            },
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
