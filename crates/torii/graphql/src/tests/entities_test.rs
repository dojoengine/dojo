#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_graphql::value;
    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};
    use tokio::sync::mpsc;
    use torii_core::State;

    use crate::tests::common::{
        entity_fixtures, init, paginate, run_graphql_query, run_graphql_subscription, Direction,
        Entity, Moves, Position,
    };

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity(pool: SqlitePool) {
        entity_fixtures(&pool).await;
        let entity_id = poseidon_hash_many(&[FieldElement::ONE]);
        let query = format!(
            r#"
            {{
                entity(id: "{:#x}") {{
                    componentNames
                }}
            }}
        "#,
            entity_id
        );
        let value = run_graphql_query(&pool, &query).await;

        let entity = value.get("entity").ok_or("no entity found").unwrap();
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.component_names, "Moves".to_string());
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity_components(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let entity_id = poseidon_hash_many(&[FieldElement::THREE]);
        let query = format!(
            r#"
                {{
                    entity (id: "{:#x}") {{
                        components {{
                            __typename
                            ... on Moves {{
                                remaining
                            }}
                            ... on Position {{
                                x
                                y
                            }}
                        }}
                    }}
                }}
            "#,
            entity_id
        );
        let value = run_graphql_query(&pool, &query).await;

        let entity = value.get("entity").ok_or("no entity found").unwrap();
        let components = entity.get("components").ok_or("no components found").unwrap();
        let component_moves: Moves = serde_json::from_value(components[0].clone()).unwrap();
        let component_position: Position = serde_json::from_value(components[1].clone()).unwrap();

        assert_eq!(component_moves.__typename, "Moves");
        assert_eq!(component_moves.remaining, 1);
        assert_eq!(component_position.__typename, "Position");
        assert_eq!(component_position.x, 69);
        assert_eq!(component_position.y, 42);
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entities_pagination(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let page_size = 2;

        // Forward pagination
        let entities_connection = paginate(&pool, None, Direction::Forward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);

        let cursor: String = entities_connection.edges[0].cursor.clone();
        let next_cursor: String = entities_connection.edges[1].cursor.clone();
        let entities_connection =
            paginate(&pool, Some(cursor), Direction::Forward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);
        assert_eq!(entities_connection.edges[0].cursor, next_cursor);

        // Backward pagination
        let entities_connection = paginate(&pool, None, Direction::Backward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);

        let cursor: String = entities_connection.edges[0].cursor.clone();
        let next_cursor: String = entities_connection.edges[1].cursor.clone();
        let entities_connection =
            paginate(&pool, Some(cursor), Direction::Backward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);
        assert_eq!(entities_connection.edges[0].cursor, next_cursor);
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity_subscription(pool: SqlitePool) {
        let state = init(&pool).await;
        // 0. Preprocess expected entity value
        let key = vec![FieldElement::ONE];
        let entity_id = format!("{:#x}", poseidon_hash_many(&key));
        let keys_str = key.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let expected_value: async_graphql::Value = value!({
                            "EntityAdded": { "id": entity_id.clone(), "keys":vec![keys_str.clone()], "componentNames": "Moves" }
        });
        let (tx, mut rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Set entity with one moves component
            // remaining: 10
            let moves_values = vec![FieldElement::from_hex_be("0xa").unwrap()];
            state.set_entity("Moves".to_string(), key, moves_values).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
            subscription {
                EntityAdded {
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

    // FIXME: Enable when `WhereInput` param implemented
    // #[sqlx::test(migrations = "../migrations")]
    // async fn test_entities_with_component_filters(pool: SqlitePool) {
    //     entity_fixtures(&pool).await;

    //     let query = "
    //     {
    //         entities (keys: [\"%%\"], componentName:\"Moves\") {
    //             keys
    //             componentNames
    //         }
    //     }
    //     ";
    //     let value = run_graphql_query(&pool, query).await;

    //     let entities = value.get("entities").ok_or("entities not found").unwrap();
    //     let entities: Vec<Entity> = serde_json::from_value(entities.clone()).unwrap();
    //     assert_eq!(entities[0].keys.clone().unwrap(), "0x1,");
    //     assert_eq!(entities[1].keys.clone().unwrap(), "0x3,");
    // }
}
