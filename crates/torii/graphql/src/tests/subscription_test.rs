#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_graphql::value;
    use dojo_types::primitive::Primitive;
    use dojo_types::schema::{Enum, Member, Struct, Ty};
    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};
    use tokio::sync::mpsc;
    // use tokio_util::sync::CancellationToken;
    use torii_core::sql::Sql;

    use crate::tests::{model_fixtures, run_graphql_subscription};

    //#[ignore]
    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity_subscription(pool: SqlitePool) {
        // Sleep in order to run this test in a single thread
        tokio::time::sleep(Duration::from_secs(1)).await;
        // let cts = CancellationToken::new();
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        model_fixtures(&mut db).await;
        // 0. Preprocess expected entity value
        let key = vec![FieldElement::ONE];
        let entity_id = format!("{:#x}", poseidon_hash_many(&key));
        let keys_str = key.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let expected_value: async_graphql::Value = value!({
                            "entityUpdated": { "id": entity_id, "keys":vec![keys_str], "model_names": "Moves" }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one moves model
            db.set_entity(
                Ty::Struct(Struct {
                    name: "Moves".to_string(),
                    children: vec![
                        Member {
                            name: "player".to_string(),
                            key: true,
                            ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::ONE))),
                        },
                        Member {
                            name: "remaining".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U8(Some(10))),
                        },
                        Member {
                            name: "last_direction".to_string(),
                            key: false,
                            ty: Ty::Enum(Enum {
                                name: "Direction".to_string(),
                                option: Some(1),
                                options: vec![
                                    ("None".to_string(), Ty::Tuple(vec![])),
                                    ("Left".to_string(), Ty::Tuple(vec![])),
                                    ("Right".to_string(), Ty::Tuple(vec![])),
                                    ("Up".to_string(), Ty::Tuple(vec![])),
                                    ("Down".to_string(), Ty::Tuple(vec![])),
                                ],
                            }),
                        },
                    ],
                }),
                &format!("0x{:064x}:0x{:04x}:0x{:04x}", 0, 0, 0),
            )
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
              entityUpdated {
                  id, keys, model_names
              }
          }"#,
        )
        .await;
        // 4. The subcription has received the message from publish()
        // 5. Compare values
        assert_eq!(expected_value, response_value);
        rx.recv().await.unwrap();
    }

    //#[ignore]
    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity_subscription_with_id(pool: SqlitePool) {
        // Sleep in order to run this test in a single thread
        tokio::time::sleep(Duration::from_secs(1)).await;
        // let cts = CancellationToken::new();
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        model_fixtures(&mut db).await;
        // 0. Preprocess expected entity value
        let key = vec![FieldElement::ONE];
        let entity_id = format!("{:#x}", poseidon_hash_many(&key));
        let keys_str = key.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let expected_value: async_graphql::Value = value!({
                                                "entityUpdated": { "id": entity_id, "keys":vec![keys_str], "model_names": "Moves" }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one moves model
            db.set_entity(
                Ty::Struct(Struct {
                    name: "Moves".to_string(),
                    children: vec![
                        Member {
                            name: "player".to_string(),
                            key: true,
                            ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::ONE))),
                        },
                        Member {
                            name: "last_direction".to_string(),
                            key: false,
                            ty: Ty::Enum(Enum {
                                name: "Direction".to_string(),
                                option: Some(1),
                                options: vec![
                                    ("None".to_string(), Ty::Tuple(vec![])),
                                    ("Left".to_string(), Ty::Tuple(vec![])),
                                    ("Right".to_string(), Ty::Tuple(vec![])),
                                    ("Up".to_string(), Ty::Tuple(vec![])),
                                    ("Down".to_string(), Ty::Tuple(vec![])),
                                ],
                            }),
                        },
                    ],
                }),
                &format!("0x{:064x}:0x{:04x}:0x{:04x}", 0, 0, 0),
            )
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
    						entityUpdated(id: "0x579e8877c7755365d5ec1ec7d3a94a457eff5d1f40482bbe9729c064cdead2") {
    								id, keys, model_names
    						}
    				}"#,
        )
        .await;
        // 4. The subscription has received the message from publish()
        // 5. Compare values
        assert_eq!(expected_value, response_value);
        rx.recv().await.unwrap();
    }

    //#[ignore]
    #[sqlx::test(migrations = "../migrations")]
    async fn test_model_subscription(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess model value
        let name = "Moves".to_string();
        let model_id = name.clone();
        let class_hash = FieldElement::TWO;
        let expected_value: async_graphql::Value = value!({
         "modelRegistered": { "id": model_id, "name":name }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let model = Ty::Struct(Struct {
                name: "Moves".to_string(),
                children: vec![Member {
                    name: "player".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                }],
            });
            db.register_model(model, vec![], class_hash, 0, 0).await.unwrap();

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

    //#[ignore]
    #[sqlx::test(migrations = "../migrations")]
    async fn test_model_subscription_with_id(pool: SqlitePool) {
        // Sleep in order to run this test at the end in a single thread
        tokio::time::sleep(Duration::from_secs(2)).await;

        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess model value
        let name = "Test".to_string();
        let model_id = name.clone();
        let class_hash = FieldElement::TWO;
        let expected_value: async_graphql::Value = value!({
         "modelRegistered": { "id": model_id, "name":name }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let model = Ty::Struct(Struct {
                name: "Test".to_string(),
                children: vec![Member {
                    name: "test".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U8(None)),
                }],
            });
            db.register_model(model, vec![], class_hash, 0, 0).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
                subscription {
                    modelRegistered(id: "Test") {
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
}
