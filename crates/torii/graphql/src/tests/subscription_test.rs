#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;

    use async_graphql::value;
    use dojo_types::primitive::Primitive;
    use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
    use serial_test::serial;
    use sqlx::SqlitePool;
    use starknet::core::types::Event;
    use starknet::core::utils::get_selector_from_name;
    use starknet_crypto::{poseidon_hash_many, FieldElement};
    use tokio::sync::mpsc;
    use torii_core::sql::Sql;

    use crate::tests::{model_fixtures, run_graphql_subscription};

    #[sqlx::test(migrations = "../migrations")]
    #[serial]
    async fn test_entity_subscription(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        model_fixtures(&mut db).await;
        // 0. Preprocess expected entity value
        let model_name = "Record".to_string();
        let key = vec![FieldElement::ONE];
        let entity_id = format!("{:#x}", poseidon_hash_many(&key));
        let keys_str = key.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let block_timestamp = 1710754478_u64;
        let expected_value: async_graphql::Value = value!({
            "entityUpdated": {
                "id": entity_id,
                "keys":vec![keys_str],
                "models" : [{
                    "__typename": model_name,
                        "depth": "Zero",
                        "record_id": 0,
                        "typeU16": 1,
                        "type_u64": "0x1",
                        "typeBool": true,
                        "type_felt": format!("{:#x}", FieldElement::from(1u128)),
                        "typeContractAddress": format!("{:#x}", FieldElement::ONE)
                }]
            }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one Record model
            db.set_entity(
                Ty::Struct(Struct {
                    name: model_name,
                    children: vec![
                        Member {
                            name: "depth".to_string(),
                            key: false,
                            ty: Ty::Enum(Enum {
                                name: "Depth".to_string(),
                                option: Some(0),
                                options: vec![
                                    EnumOption { name: "Zero".to_string(), ty: Ty::Tuple(vec![]) },
                                    EnumOption { name: "One".to_string(), ty: Ty::Tuple(vec![]) },
                                    EnumOption { name: "Two".to_string(), ty: Ty::Tuple(vec![]) },
                                    EnumOption { name: "Three".to_string(), ty: Ty::Tuple(vec![]) },
                                ],
                            }),
                        },
                        Member {
                            name: "record_id".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U8(Some(0))),
                        },
                        Member {
                            name: "typeU16".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U16(Some(1))),
                        },
                        Member {
                            name: "type_u64".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U64(Some(1))),
                        },
                        Member {
                            name: "typeBool".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::Bool(Some(true))),
                        },
                        Member {
                            name: "type_felt".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::Felt252(Some(FieldElement::from(1u128)))),
                        },
                        Member {
                            name: "typeContractAddress".to_string(),
                            key: true,
                            ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::ONE))),
                        },
                    ],
                }),
                &format!("0x{:064x}:0x{:04x}:0x{:04x}", 0, 0, 0),
                block_timestamp,
            )
            .await
            .unwrap();

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listening, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"subscription {
                entityUpdated {
                    id 
                    keys
                    models {
                        __typename
                        ... on Record {
                            depth
                            record_id
                            typeU16
                            type_u64
                            typeBool
                            type_felt
                            typeContractAddress
                        }
                    }
                }
            }"#,
        )
        .await;
        // 4. The subscription has received the message from publish()
        // 5. Compare values
        assert_eq!(expected_value, response_value);
        rx.recv().await.unwrap();
    }

    #[sqlx::test(migrations = "../migrations")]
    #[serial]
    async fn test_entity_subscription_with_id(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        model_fixtures(&mut db).await;
        // 0. Preprocess expected entity value
        let model_name = "Record".to_string();
        let key = vec![FieldElement::ONE];
        let entity_id = format!("{:#x}", poseidon_hash_many(&key));
        let block_timestamp = 1710754478_u64;
        let keys_str = key.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let expected_value: async_graphql::Value = value!({
            "entityUpdated": {
                "id": entity_id,
                "keys":vec![keys_str],
                "models" : [{
                    "__typename": model_name,
                        "depth": "Zero",
                        "record_id": 0,
                        "type_felt": format!("{:#x}", FieldElement::from(1u128)),
                        "typeContractAddress": format!("{:#x}", FieldElement::ONE)
                }]
            }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one Record model
            db.set_entity(
                Ty::Struct(Struct {
                    name: model_name,
                    children: vec![
                        Member {
                            name: "depth".to_string(),
                            key: false,
                            ty: Ty::Enum(Enum {
                                name: "Depth".to_string(),
                                option: Some(0),
                                options: vec![
                                    EnumOption { name: "Zero".to_string(), ty: Ty::Tuple(vec![]) },
                                    EnumOption { name: "One".to_string(), ty: Ty::Tuple(vec![]) },
                                    EnumOption { name: "Two".to_string(), ty: Ty::Tuple(vec![]) },
                                    EnumOption { name: "Three".to_string(), ty: Ty::Tuple(vec![]) },
                                ],
                            }),
                        },
                        Member {
                            name: "record_id".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U32(Some(0))),
                        },
                        Member {
                            name: "type_felt".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::Felt252(Some(FieldElement::from(1u128)))),
                        },
                        Member {
                            name: "typeContractAddress".to_string(),
                            key: true,
                            ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::ONE))),
                        },
                    ],
                }),
                &format!("0x{:064x}:0x{:04x}:0x{:04x}", 0, 0, 0),
                block_timestamp,
            )
            .await
            .unwrap();

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listening, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"subscription {
                entityUpdated(id: "0x579e8877c7755365d5ec1ec7d3a94a457eff5d1f40482bbe9729c064cdead2") {
                    id 
                    keys
                    models {
                        __typename
                        ... on Record {
                            depth
                            record_id
                            type_felt
                            typeContractAddress
                        }
                    }
                }
            }"#,
        )
        .await;
        // 4. The subscription has received the message from publish()
        // 5. Compare values
        assert_eq!(expected_value, response_value);
        rx.recv().await.unwrap();
    }

    #[sqlx::test(migrations = "../migrations")]
    #[serial]
    async fn test_model_subscription(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess model value
        let model_name = "Subrecord".to_string();
        let model_id = format!("{:#x}", get_selector_from_name(&model_name).unwrap());
        let class_hash = FieldElement::TWO;
        let contract_address = FieldElement::THREE;
        let block_timestamp: u64 = 1710754478_u64;
        let expected_value: async_graphql::Value = value!({
         "modelRegistered": { "id": model_id, "name":model_name }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let model = Ty::Struct(Struct {
                name: model_name,
                children: vec![Member {
                    name: "subrecordId".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::U32(None)),
                }],
            });
            db.register_model(model, vec![], class_hash, contract_address, 0, 0, block_timestamp)
                .await
                .unwrap();

            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
                subscription {
                    modelRegistered {
                            id, name
                        }
                }"#,
        )
        .await;
        // 4. The subcription has received the message from publish()
        // 5. Compare values
        assert_eq!(expected_value, response_value);
        rx.recv().await.unwrap();
    }

    #[sqlx::test(migrations = "../migrations")]
    #[serial]
    async fn test_model_subscription_with_id(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess model value
        let model_name = "Subrecord".to_string();
        let model_id = format!("{:#x}", get_selector_from_name(&model_name).unwrap());
        let class_hash = FieldElement::TWO;
        let contract_address = FieldElement::THREE;
        let block_timestamp: u64 = 1710754478_u64;
        let expected_value: async_graphql::Value = value!({
         "modelRegistered": { "id": model_id, "name":model_name }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let model = Ty::Struct(Struct {
                name: model_name,
                children: vec![Member {
                    name: "type_u8".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U8(None)),
                }],
            });
            db.register_model(model, vec![], class_hash, contract_address, 0, 0, block_timestamp)
                .await
                .unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            &format!(
                r#"
            subscription {{
                modelRegistered(id: "{}") {{
                        id, name
                    }}
            }}"#,
                model_id
            ),
        )
        .await;
        // 4. The subcription has received the message from publish()
        // 5. Compare values
        assert_eq!(expected_value, response_value);
        rx.recv().await.unwrap();
    }

    #[sqlx::test(migrations = "../migrations")]
    #[serial]
    async fn test_event_emitted(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        let block_timestamp: u64 = 1710754478_u64;
        let (tx, mut rx) = mpsc::channel(7);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;

            db.store_event(
                "0x0",
                &Event {
                    from_address: FieldElement::ZERO,
                    keys: vec![
                        FieldElement::from_str("0xdead").unwrap(),
                        FieldElement::from_str("0xbeef").unwrap(),
                    ],
                    data: vec![
                        FieldElement::from_str("0xc0de").unwrap(),
                        FieldElement::from_str("0xface").unwrap(),
                    ],
                },
                FieldElement::ZERO,
                block_timestamp,
            );

            tx.send(()).await.unwrap();
        });

        let response_value = run_graphql_subscription(
            &pool,
            &format!(
                r#"
                    subscription {{
                        eventEmitted (keys: ["*", "{:#x}"]) {{
                            keys
                            data
                            transactionHash
                        }}
                    }}
                "#,
                FieldElement::from_str("0xbeef").unwrap()
            ),
        )
        .await;

        let expected_value: async_graphql::Value = value!({
         "eventEmitted": { "keys": vec![
            format!("{:#x}", FieldElement::from_str("0xdead").unwrap()),
            format!("{:#x}", FieldElement::from_str("0xbeef").unwrap())
         ], "data": vec![
            format!("{:#x}", FieldElement::from_str("0xc0de").unwrap()),
            format!("{:#x}", FieldElement::from_str("0xface").unwrap())
         ], "transactionHash": format!("{:#x}", FieldElement::ZERO)}
        });

        assert_eq!(response_value, expected_value);
        rx.recv().await.unwrap();
    }
}
