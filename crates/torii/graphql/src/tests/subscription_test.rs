#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_graphql::value;
    use dojo_world::manifest::{Component, Member};
    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};
    use tokio::sync::mpsc;
    use torii_core::sql::Sql;
    use torii_core::State;

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
                            "entityUpdated": { "id": entity_id.clone(), "keys":vec![keys_str.clone()], "componentNames": "Moves" }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one moves component
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
                  id, keys, componentNames
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
    async fn test_component_subscription(pool: SqlitePool) {
        // Sleep in order to run this test at the end in a single thread
        tokio::time::sleep(Duration::from_secs(2)).await;

        let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess component value
        let name = "Test".to_string();
        let component_id = name.to_lowercase();
        let class_hash = FieldElement::TWO;
        let hex_class_hash = format!("{:#x}", class_hash);
        let expected_value: async_graphql::Value = value!({
         "componentRegistered": { "id": component_id.clone(), "name":name, "classHash": hex_class_hash }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let component = Component {
                name,
                members: vec![Member { name: "test".into(), ty: "u32".into(), key: false }],
                class_hash,
                ..Default::default()
            };
            state.register_component(component).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
            subscription {
                componentRegistered {
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
