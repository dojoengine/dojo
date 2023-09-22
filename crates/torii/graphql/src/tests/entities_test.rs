#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};

    use crate::tests::common::{
        entity_fixtures, paginate, run_graphql_query, Entity, Moves, Paginate, Position,
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

    #[ignore]
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
                                last_direction
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
        let entities_connection = paginate(&pool, None, Paginate::Forward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);

        let cursor: String = entities_connection.edges[0].cursor.clone();
        let next_cursor: String = entities_connection.edges[1].cursor.clone();
        let entities_connection = paginate(&pool, Some(cursor), Paginate::Forward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);
        assert_eq!(entities_connection.edges[0].cursor, next_cursor);

        // Backward pagination
        let entities_connection = paginate(&pool, None, Paginate::Backward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);

        let cursor: String = entities_connection.edges[0].cursor.clone();
        let next_cursor: String = entities_connection.edges[1].cursor.clone();
        let entities_connection =
            paginate(&pool, Some(cursor), Paginate::Backward, page_size).await;
        assert_eq!(entities_connection.total_count, 3);
        assert_eq!(entities_connection.edges.len(), page_size);
        assert_eq!(entities_connection.edges[0].cursor, next_cursor);
    }
}
