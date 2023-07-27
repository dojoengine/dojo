#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use sqlx::SqlitePool;
    use starknet_crypto::{poseidon_hash_many, FieldElement};

    use crate::tests::common::{entity_fixtures, run_graphql_query, Entity, Moves, Position};

    #[derive(Deserialize)]
    struct PositionResult {
        entity_id: String,
        entity: Entity,
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_component_no_filter(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let query = r#"
                {
                    movesComponents {
                        __typename
                        remaining
                    }
                    positionComponents {
                        __typename
                        x
                        y
                    }
                }
            "#;

        let value = run_graphql_query(&pool, query).await;

        let moves_list = value.get("movesComponents").ok_or("no moves found").unwrap();
        let moves_list: Vec<Moves> = serde_json::from_value(moves_list.clone()).unwrap();
        let position_list = value.get("positionComponents").ok_or("no position found").unwrap();
        let position_list: Vec<Position> = serde_json::from_value(position_list.clone()).unwrap();

        assert_eq!(moves_list[0].remaining, 10);
        assert_eq!(position_list[0].x, 42);
        assert_eq!(position_list[0].y, 69);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_component_filter(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let query = r#"
                {
                    positionComponents (x: 42) {
                        __typename
                        x
                        y
                    }
                }
            "#;
        let value = run_graphql_query(&pool, query).await;

        let positions = value.get("positionComponents").ok_or("no positions found").unwrap();
        let positions: Vec<Position> = serde_json::from_value(positions.clone()).unwrap();
        assert_eq!(positions[0].y, 69);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_component_entity_relationship(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let query = r#"
                {
                    positionComponents  {
                        entity_id
                        entity {
                            keys
                            componentNames
                        }
                    }
                }
            "#;
        let value = run_graphql_query(&pool, query).await;

        let positions = value.get("positionComponents").ok_or("no positions found").unwrap();
        let positions: Vec<PositionResult> = serde_json::from_value(positions.clone()).unwrap();
        let entity_id = poseidon_hash_many(&[FieldElement::TWO]);
        assert_eq!(positions[0].entity_id, format!("{:#x}", entity_id));
        assert_eq!(positions[0].entity.component_names, "Position".to_string());
    }
}
