#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use serde::Deserialize;
    use sqlx::SqlitePool;

    use crate::state::sql::Sql;
    use crate::state::State;
    use crate::tests::common::run_graphql_query;

    #[derive(Deserialize)]
    struct Moves {
        __typename: String,
        remaining: i64,
    }

    #[derive(Deserialize)]
    struct Position {
        __typename: String,
        x: i64,
        y: i64,
    }

    #[derive(Deserialize)]
    struct ComponentMoves {
        name: String,
        storage: Moves,
    }

    #[derive(Deserialize)]
    struct ComponentPosition {
        name: String,
        storage: Position,
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_storage_components(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = r#"
                { 
                    moves(id: 1) { 
                        __typename
                        remaining 
                    } 
                    position(id: 1) { 
                        __typename
                        x 
                        y 
                    }
                }
            "#;
        let value = run_graphql_query(&pool, query).await;

        let moves = value.get("moves").ok_or("no moves found").unwrap();
        let moves: Moves = serde_json::from_value(moves.clone()).unwrap();
        let position = value.get("position").ok_or("no position found").unwrap();
        let position: Position = serde_json::from_value(position.clone()).unwrap();

        assert_eq!(moves.remaining, 10);
        assert_eq!(position.x, 42);
        assert_eq!(position.y, 69);
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_storage_union(pool: SqlitePool) {
        let manifest = dojo_world::manifest::Manifest::load_from_path(
            Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev/manifest.json".into())
                .unwrap(),
        )
        .unwrap();

        let mut state = Sql::new(pool.clone()).unwrap();
        state.load_from_manifest(manifest).await.unwrap();

        let _ = pool.acquire().await;

        let query = r#"
                { 
                    component_moves: component(id: "component_1") {
                        name
                        storage {
                            __typename
                            ... on Moves {
                                remaining
                            }
                        }
                    }
                    component_position: component(id: "component_2") {
                        name
                        storage {
                            __typename
                            ... on Position {
                                x
                                y
                            }
                        }
                    }
                }
            "#;
        let value = run_graphql_query(&pool, query).await;
        let component_moves = value.get("component_moves").ok_or("no component found").unwrap();
        let component_moves: ComponentMoves =
            serde_json::from_value(component_moves.clone()).unwrap();
        let component_position =
            value.get("component_position").ok_or("no component found").unwrap();
        let component_position: ComponentPosition =
            serde_json::from_value(component_position.clone()).unwrap();

        assert_eq!(component_moves.name, component_moves.storage.__typename);
        assert_eq!(component_position.name, component_position.storage.__typename);
    }
}
