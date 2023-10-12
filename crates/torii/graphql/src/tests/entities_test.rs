#[cfg(test)]
mod tests {

    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};
    use torii_core::sql::Sql;

    use crate::tests::{
        cursor_paginate, entity_fixtures, offset_paginate, run_graphql_query, Entity, Moves,
        Paginate, Position,
    };

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        entity_fixtures(&mut db).await;

        let entity_id = poseidon_hash_many(&[FieldElement::ONE]);
        println!("{:#x}", entity_id);
        let query = format!(
            r#"
            {{
                entity(id: "{:#x}") {{
                    model_names
                }}
            }}
        "#,
            entity_id
        );
        let value = run_graphql_query(&pool, &query).await;

        let entity = value.get("entity").ok_or("no entity found").unwrap();
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.model_names, "Moves".to_string());
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entity_models(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        entity_fixtures(&mut db).await;

        let entity_id = poseidon_hash_many(&[FieldElement::THREE]);
        let query = format!(
            r#"
                {{
                    entity (id: "{:#x}") {{
                        models {{
                            __typename
                            ... on Moves {{
                                remaining
                                last_direction
                            }}
                            ... on Position {{
                                vec {{
                                    x
                                    y
                                }}
                            }}
                        }}
                    }}
                }}
            "#,
            entity_id
        );
        let value = run_graphql_query(&pool, &query).await;

        let entity = value.get("entity").ok_or("no entity found").unwrap();
        let models = entity.get("models").ok_or("no models found").unwrap();
        let model_moves: Moves = serde_json::from_value(models[0].clone()).unwrap();
        let model_position: Position = serde_json::from_value(models[1].clone()).unwrap();

        assert_eq!(model_moves.__typename, "Moves");
        assert_eq!(model_moves.remaining, 10);
        assert_eq!(model_position.__typename, "Position");
        assert_eq!(model_position.vec.x, 42);
        assert_eq!(model_position.vec.y, 69);
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entities_cursor_pagination(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        entity_fixtures(&mut db).await;

        let page_size = 2;

        // Forward pagination
        let entities_connection = cursor_paginate(&pool, None, Paginate::Forward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);

        let cursor: String = entities_connection.edges[0].cursor.clone();
        let next_cursor: String = entities_connection.edges[1].cursor.clone();
        let entities_connection =
            cursor_paginate(&pool, Some(cursor), Paginate::Forward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);
        assert_eq!(entities_connection.edges[0].cursor, next_cursor);

        // Backward pagination
        let entities_connection = cursor_paginate(&pool, None, Paginate::Backward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);

        let cursor: String = entities_connection.edges[0].cursor.clone();
        let next_cursor: String = entities_connection.edges[1].cursor.clone();
        let entities_connection =
            cursor_paginate(&pool, Some(cursor), Paginate::Backward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);
        assert_eq!(entities_connection.edges[0].cursor, next_cursor);
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_entities_offset_pagination(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        entity_fixtures(&mut db).await;

        let limit = 3;
        let mut offset = 0;
        let entities_connection = offset_paginate(&pool, offset, limit).await;
        let offset_plus_one = entities_connection.edges[1].node.model_names.clone();
        let offset_plus_two = entities_connection.edges[2].node.model_names.clone();
        assert_eq!(entities_connection.edges.len(), 3);

        offset = 1;
        let entities_connection = offset_paginate(&pool, offset, limit).await;
        assert_eq!(entities_connection.edges[0].node.model_names, offset_plus_one);
        assert_eq!(entities_connection.edges.len(), 2);

        offset = 2;
        let entities_connection = offset_paginate(&pool, offset, limit).await;
        assert_eq!(entities_connection.edges[0].node.model_names, offset_plus_two);
        assert_eq!(entities_connection.edges.len(), 1);
    }
}
