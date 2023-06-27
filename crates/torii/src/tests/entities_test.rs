#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};

    use crate::tests::common::{entity_fixtures, run_graphql_query};

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Entity {
        pub component_names: String,
    }

    #[derive(Deserialize)]
    struct Moves {
        __typename: String,
        remaining: u32,
    }

    #[derive(Deserialize)]
    struct Position {
        __typename: String,
        x: u32,
        y: u32,
    }

    #[sqlx::test(migrations = "./migrations")]
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

    #[sqlx::test(migrations = "./migrations")]
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
        assert_eq!(component_moves.remaining, 10);
        assert_eq!(component_position.__typename, "Position");
        assert_eq!(component_position.x, 42);
        assert_eq!(component_position.y, 69);
    }
}
