#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_graphql::value;
    use dojo_world::manifest::{Member, Model};
    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};
    use tokio::sync::mpsc;
    use torii_core::sql::Sql;

    use crate::tests::common::{init, run_graphql_subscription};

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity_subscription(pool: SqlitePool) {
        // Sleep in order to run this test in a single thread
        tokio::time::sleep(Duration::from_secs(1)).await;
        let state = init(&pool).await;
        // 0. Preprocess expected entity value
        let key = vec![FieldElement::ONE];
        let entity_id = format!("{:#x}", poseidon_hash_many(&key));
        let keys_str = key.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let expected_value: async_graphql::Value = value!({
                            "entityUpdated": { "id": entity_id.clone(), "keys":vec![keys_str.clone()], "modelNames": "Moves" }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one moves model
            // remaining: 10, last_direction: 0
            let moves_values = vec![FieldElement::from_hex_be("0xa").unwrap(), FieldElement::ZERO];
            state.set_entity("Moves".to_string(), key, moves_values).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
          subscription {
              entityUpdated {
                  id, keys, modelNames
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
    async fn test_entity_subscription_with_id(pool: SqlitePool) {
        // Sleep in order to run this test in a single thread
        tokio::time::sleep(Duration::from_secs(1)).await;
        let state = init(&pool).await;
        // 0. Preprocess expected entity value
        let key = vec![FieldElement::ONE];
        let entity_id = format!("{:#x}", poseidon_hash_many(&key));
        let keys_str = key.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let expected_value: async_graphql::Value = value!({
                                                "entityUpdated": { "id": entity_id.clone(), "keys":vec![keys_str.clone()], "modelNames": "Moves" }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one moves model
            // remaining: 10, last_direction: 0
            let moves_values = vec![FieldElement::from_hex_be("0xa").unwrap(), FieldElement::ZERO];
            state.set_entity("Moves".to_string(), key, moves_values).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
				subscription {
						entityUpdated(id: "0x579e8877c7755365d5ec1ec7d3a94a457eff5d1f40482bbe9729c064cdead2") {
								id, keys, modelNames
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
    async fn test_model_subscription(pool: SqlitePool) {
        // Sleep in order to run this test at the end in a single thread
        tokio::time::sleep(Duration::from_secs(2)).await;

        let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess model value
        let name = "Test".to_string();
        let model_id = name.to_lowercase();
        let class_hash = FieldElement::TWO;
        let hex_class_hash = format!("{:#x}", class_hash);
        let expected_value: async_graphql::Value = value!({
         "modelRegistered": { "id": model_id.clone(), "name":name, "classHash": hex_class_hash }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let model = Model {
                name,
                members: vec![Member { name: "test".into(), ty: "u32".into(), key: false }],
                class_hash,
                ..Default::default()
            };
            state.register_model(model).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
            subscription {
                modelRegistered {
                        id, name, classHash
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
    async fn test_model_subscription_with_id(pool: SqlitePool) {
        // Sleep in order to run this test at the end in a single thread
        tokio::time::sleep(Duration::from_secs(2)).await;

        let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess model value
        let name = "Test".to_string();
        let model_id = name.to_lowercase();
        let class_hash = FieldElement::TWO;
        let hex_class_hash = format!("{:#x}", class_hash);
        let expected_value: async_graphql::Value = value!({
         "modelRegistered": { "id": model_id.clone(), "name":name, "classHash": hex_class_hash }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let model = Model {
                name,
                members: vec![Member { name: "test".into(), ty: "u32".into(), key: false }],
                class_hash,
                ..Default::default()
            };
            state.register_model(model).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
            subscription {
                modelRegistered(id: "test") {
                        id, name, classHash
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
