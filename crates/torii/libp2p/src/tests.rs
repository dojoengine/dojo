#[cfg(test)]
mod test {
    use std::error::Error;

    use crate::client::RelayClient;
    use crate::typed_data::{
        map_ty_to_primitive, parse_value_to_ty, Domain, PrimitiveType, TypedData,
    };

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
    use crypto_bigint::U256;
    use dojo_types::primitive::Primitive;
    use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
    use katana_runner::KatanaRunner;
    use serde_json::Number;
    use starknet::core::types::Felt;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    fn test_parse_primitive_to_ty() {
        // primitives
        let mut ty = Ty::Primitive(Primitive::U8(None));
        let value = PrimitiveType::Number(Number::from(1u64));
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::U8(Some(1))));

        let mut ty = Ty::Primitive(Primitive::U16(None));
        let value = PrimitiveType::Number(Number::from(1u64));
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::U16(Some(1))));

        let mut ty = Ty::Primitive(Primitive::U32(None));
        let value = PrimitiveType::Number(Number::from(1u64));
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::U32(Some(1))));

        let mut ty = Ty::Primitive(Primitive::USize(None));
        let value = PrimitiveType::Number(Number::from(1u64));
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::USize(Some(1))));

        let mut ty = Ty::Primitive(Primitive::U64(None));
        let value = PrimitiveType::Number(Number::from(1u64));
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::U64(Some(1))));

        let mut ty = Ty::Primitive(Primitive::U128(None));
        let value = PrimitiveType::String("1".to_string());
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::U128(Some(1))));

        // test u256 with low high
        let mut ty = Ty::Primitive(Primitive::U256(None));
        let value = PrimitiveType::Object(
            vec![
                ("low".to_string(), PrimitiveType::String("1".to_string())),
                ("high".to_string(), PrimitiveType::String("0".to_string())),
            ]
            .into_iter()
            .collect(),
        );
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::U256(Some(U256::ONE))));

        let mut ty = Ty::Primitive(Primitive::Felt252(None));
        let value = PrimitiveType::String("1".to_string());
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::Felt252(Some(Felt::ONE))));

        let mut ty = Ty::Primitive(Primitive::ClassHash(None));
        let value = PrimitiveType::String("1".to_string());
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::ClassHash(Some(Felt::ONE))));

        let mut ty = Ty::Primitive(Primitive::ContractAddress(None));
        let value = PrimitiveType::String("1".to_string());
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))));

        let mut ty = Ty::Primitive(Primitive::Bool(None));
        let value = PrimitiveType::Bool(true);
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::Primitive(Primitive::Bool(Some(true))));

        // bytearray
        let mut ty = Ty::ByteArray("".to_string());
        let value = PrimitiveType::String("mimi".to_string());
        parse_value_to_ty(&value, &mut ty).unwrap();
        assert_eq!(ty, Ty::ByteArray("mimi".to_string()));
    }

    #[test]
    fn test_map_ty_to_primitive() {
        let ty = Ty::Primitive(Primitive::U8(Some(1)));
        let value = PrimitiveType::Number(Number::from(1u64));
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::U16(Some(1)));
        let value = PrimitiveType::Number(Number::from(1u64));
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::U32(Some(1)));
        let value = PrimitiveType::Number(Number::from(1u64));
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::USize(Some(1)));
        let value = PrimitiveType::Number(Number::from(1u64));
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::U64(Some(1)));
        let value = PrimitiveType::String("1".to_string());
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::U128(Some(1)));
        let value = PrimitiveType::String("1".to_string());
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::U256(Some(U256::ONE)));
        let value = PrimitiveType::Object(
            vec![
                ("low".to_string(), PrimitiveType::String("1".to_string())),
                ("high".to_string(), PrimitiveType::String("0".to_string())),
            ]
            .into_iter()
            .collect(),
        );
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::Felt252(Some(Felt::ONE)));
        let value = PrimitiveType::String("1".to_string());
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::ClassHash(Some(Felt::ONE)));
        let value = PrimitiveType::String("1".to_string());
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE)));
        let value = PrimitiveType::String("1".to_string());
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::Primitive(Primitive::Bool(Some(true)));
        let value = PrimitiveType::Bool(true);
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

        let ty = Ty::ByteArray("mimi".to_string());
        let value = PrimitiveType::String("mimi".to_string());
        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());
    }

    #[test]
    fn test_parse_complex_to_ty() {
        let mut ty = Ty::Struct(Struct {
            name: "PlayerConfig".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                    key: true,
                },
                Member { name: "name".to_string(), ty: Ty::ByteArray("".to_string()), key: false },
                Member {
                    name: "items".to_string(),
                    // array of PlayerItem struct
                    ty: Ty::Array(vec![Ty::Struct(Struct {
                        name: "PlayerItem".to_string(),
                        children: vec![
                            Member {
                                name: "item_id".to_string(),
                                ty: Ty::Primitive(Primitive::U32(None)),
                                key: false,
                            },
                            Member {
                                name: "quantity".to_string(),
                                ty: Ty::Primitive(Primitive::U32(None)),
                                key: false,
                            },
                        ],
                    })]),
                    key: false,
                },
                // a favorite_item field with enum type Option<PlayerItem>
                Member {
                    name: "favorite_item".to_string(),
                    ty: Ty::Enum(Enum {
                        name: "Option".to_string(),
                        option: None,
                        options: vec![
                            EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption {
                                name: "Some".to_string(),
                                ty: Ty::Struct(Struct {
                                    name: "PlayerItem".to_string(),
                                    children: vec![
                                        Member {
                                            name: "item_id".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(None)),
                                            key: false,
                                        },
                                        Member {
                                            name: "quantity".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(None)),
                                            key: false,
                                        },
                                    ],
                                }),
                            },
                        ],
                    }),
                    key: false,
                },
            ],
        });

        let value = PrimitiveType::Object(
            vec![
                ("player".to_string(), PrimitiveType::String("1".to_string())),
                ("name".to_string(), PrimitiveType::String("mimi".to_string())),
                (
                    "items".to_string(),
                    PrimitiveType::Array(vec![PrimitiveType::Object(
                        vec![
                            ("item_id".to_string(), PrimitiveType::String("1".to_string())),
                            ("quantity".to_string(), PrimitiveType::Number(Number::from(1u64))),
                        ]
                        .into_iter()
                        .collect(),
                    )]),
                ),
                (
                    "favorite_item".to_string(),
                    PrimitiveType::Object(
                        vec![(
                            "Some".to_string(),
                            PrimitiveType::Object(
                                vec![
                                    ("item_id".to_string(), PrimitiveType::String("1".to_string())),
                                    (
                                        "quantity".to_string(),
                                        PrimitiveType::Number(Number::from(1u64)),
                                    ),
                                ]
                                .into_iter()
                                .collect(),
                            ),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        );

        parse_value_to_ty(&value, &mut ty).unwrap();

        assert_eq!(
            ty,
            Ty::Struct(Struct {
                name: "PlayerConfig".to_string(),
                children: vec![
                    Member {
                        name: "player".to_string(),
                        ty: Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))),
                        key: true,
                    },
                    Member {
                        name: "name".to_string(),
                        ty: Ty::ByteArray("mimi".to_string()),
                        key: false,
                    },
                    Member {
                        name: "items".to_string(),
                        ty: Ty::Array(vec![Ty::Struct(Struct {
                            name: "PlayerItem".to_string(),
                            children: vec![
                                Member {
                                    name: "item_id".to_string(),
                                    ty: Ty::Primitive(Primitive::U32(Some(1))),
                                    key: false,
                                },
                                Member {
                                    name: "quantity".to_string(),
                                    ty: Ty::Primitive(Primitive::U32(Some(1))),
                                    key: false,
                                },
                            ],
                        })]),
                        key: false,
                    },
                    Member {
                        name: "favorite_item".to_string(),
                        ty: Ty::Enum(Enum {
                            name: "Option".to_string(),
                            option: Some(1_u8),
                            options: vec![
                                EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                                EnumOption {
                                    name: "Some".to_string(),
                                    ty: Ty::Struct(Struct {
                                        name: "PlayerItem".to_string(),
                                        children: vec![
                                            Member {
                                                name: "item_id".to_string(),
                                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                                key: false,
                                            },
                                            Member {
                                                name: "quantity".to_string(),
                                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                                key: false,
                                            },
                                        ],
                                    }),
                                },
                            ]
                        }),
                        key: false,
                    },
                ],
            })
        );
    }

    #[test]
    fn test_map_ty_to_complex() {
        let ty = Ty::Struct(Struct {
            name: "PlayerConfig".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))),
                    key: true,
                },
                Member {
                    name: "name".to_string(),
                    ty: Ty::ByteArray("mimi".to_string()),
                    key: false,
                },
                Member {
                    name: "items".to_string(),
                    ty: Ty::Array(vec![Ty::Struct(Struct {
                        name: "PlayerItem".to_string(),
                        children: vec![
                            Member {
                                name: "item_id".to_string(),
                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                key: false,
                            },
                            Member {
                                name: "quantity".to_string(),
                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                key: false,
                            },
                        ],
                    })]),
                    key: false,
                },
                Member {
                    name: "favorite_item".to_string(),
                    ty: Ty::Enum(Enum {
                        name: "Option".to_string(),
                        option: Some(1_u8),
                        options: vec![
                            EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption {
                                name: "Some".to_string(),
                                ty: Ty::Struct(Struct {
                                    name: "PlayerItem".to_string(),
                                    children: vec![
                                        Member {
                                            name: "item_id".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                                            key: false,
                                        },
                                        Member {
                                            name: "quantity".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                                            key: false,
                                        },
                                    ],
                                }),
                            },
                        ],
                    }),
                    key: false,
                },
            ],
        });

        let value = PrimitiveType::Object(
            vec![
                ("player".to_string(), PrimitiveType::String("1".to_string())),
                ("name".to_string(), PrimitiveType::String("mimi".to_string())),
                (
                    "items".to_string(),
                    PrimitiveType::Array(vec![PrimitiveType::Object(
                        vec![
                            ("item_id".to_string(), PrimitiveType::Number(Number::from(1u64))),
                            ("quantity".to_string(), PrimitiveType::Number(Number::from(1u64))),
                        ]
                        .into_iter()
                        .collect(),
                    )]),
                ),
                (
                    "favorite_item".to_string(),
                    PrimitiveType::Object(
                        vec![(
                            "Some".to_string(),
                            PrimitiveType::Object(
                                vec![
                                    (
                                        "item_id".to_string(),
                                        PrimitiveType::Number(Number::from(1u64)),
                                    ),
                                    (
                                        "quantity".to_string(),
                                        PrimitiveType::Number(Number::from(1u64)),
                                    ),
                                ]
                                .into_iter()
                                .collect(),
                            ),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        );

        assert_eq!(value, map_ty_to_primitive(&ty).unwrap());
    }

    #[test]
    fn test_model_to_typed_data() {
        let ty = Ty::Struct(Struct {
            name: "PlayerConfig".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))),
                    key: true,
                },
                Member {
                    name: "name".to_string(),
                    ty: Ty::ByteArray("mimi".to_string()),
                    key: false,
                },
                Member {
                    name: "items".to_string(),
                    // array of PlayerItem struct
                    ty: Ty::Array(vec![Ty::Struct(Struct {
                        name: "PlayerItem".to_string(),
                        children: vec![
                            Member {
                                name: "item_id".to_string(),
                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                key: false,
                            },
                            Member {
                                name: "quantity".to_string(),
                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                key: false,
                            },
                        ],
                    })]),
                    key: false,
                },
                // a favorite_item field with enum type Option<PlayerItem>
                Member {
                    name: "favorite_item".to_string(),
                    ty: Ty::Enum(Enum {
                        name: "Option".to_string(),
                        option: Some(1),
                        options: vec![
                            EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption {
                                name: "Some".to_string(),
                                ty: Ty::Struct(Struct {
                                    name: "PlayerItem".to_string(),
                                    children: vec![
                                        Member {
                                            name: "item_id".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(Some(69))),
                                            key: false,
                                        },
                                        Member {
                                            name: "quantity".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(Some(42))),
                                            key: false,
                                        },
                                    ],
                                }),
                            },
                        ],
                    }),
                    key: false,
                },
            ],
        });

        let typed_data =
            TypedData::from_model(ty, Domain::new("Test", "1", "Test", Some("1"))).unwrap();

        let path = "mocks/model_PlayerConfig.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let file_typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        assert_eq!(
            typed_data.encode(Felt::ZERO).unwrap(),
            file_typed_data.encode(Felt::ZERO).unwrap()
        );
    }

    // This tests subscribing to a topic and receiving a message
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_client_messaging() -> Result<(), Box<dyn Error>> {
        use std::collections::HashMap;
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
        use torii_core::sql::Sql;
        use torii_core::types::ContractType;

        use crate::server::Relay;
        use crate::typed_data::{Domain, Field, SimpleField, TypedData};
        use crate::types::{Message, Signature};

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

        let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));

        let account = sequencer.account_data(0);

        let (shutdown_tx, _) = broadcast::channel(1);
        let (mut executor, sender) =
            Executor::new(pool.clone(), shutdown_tx.clone()).await.unwrap();
        tokio::spawn(async move {
            executor.run().await.unwrap();
        });
        let mut db =
            Sql::new(pool.clone(), sender, &HashMap::from([(Felt::ZERO, ContractType::WORLD)]))
                .await
                .unwrap();

        // Register the model of our Message
        db.register_model(
            "types_test",
            Ty::Struct(Struct {
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
            crate::typed_data::PrimitiveType::String(account.address.to_string()),
        );

        typed_data.message.insert(
            "message".to_string(),
            crate::typed_data::PrimitiveType::String("mimi".to_string()),
        );

        let message_hash = typed_data.encode(account.address).unwrap();
        let signature =
            SigningKey::from_secret_scalar(account.private_key.clone().unwrap().secret_scalar())
                .sign(&message_hash)
                .unwrap();

        client
            .command_sender
            .publish(Message { message: typed_data, signature: Signature::Starknet((signature.r, signature.s)) })
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
}
