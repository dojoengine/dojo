#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use serde::Deserialize;
    use sqlx::{FromRow, SqlitePool};

    use crate::tests::common::{entity_fixtures, run_graphql_query};

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

    #[derive(FromRow, Deserialize)]
    pub struct Component {
        pub id: String,
        pub name: String,
        pub class_hash: String,
        pub transaction_hash: String,
        pub created_at: DateTime<Utc>,
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
}
