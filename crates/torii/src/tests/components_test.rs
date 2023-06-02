#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use sqlx::SqlitePool;

    use crate::tests::common::run_graphql_query;

    #[derive(Deserialize)]
    struct Game {
        __typename: String,
        name: String,
        is_finished: bool,
    }

    #[derive(Deserialize)]
    struct Stats {
        __typename: String,
        health: i64,
        mana: i64,
    }

    #[derive(Deserialize)]
    struct ComponentGame {
        name: String,
        storage: Game,
    }

    #[derive(Deserialize)]
    struct ComponentStats {
        name: String,
        storage: Stats,
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities", "components"))]
    async fn test_storage_components(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = r#"
                { 
                    game(id: 1) { 
                        __typename
                        name 
                        is_finished 
                    } 
                    stats(id: 1) { 
                        __typename
                        health 
                        mana 
                    } 
                }
            "#;
        let value = run_graphql_query(&pool, query).await;

        let game = value.get("game").ok_or("no game found").unwrap();
        let game: Game = serde_json::from_value(game.clone()).unwrap();
        let stats = value.get("stats").ok_or("no stats found").unwrap();
        let stats: Stats = serde_json::from_value(stats.clone()).unwrap();

        assert!(!game.is_finished);
        assert_eq!(game.name, "0x594F4C4F");
        assert_eq!(stats.health, 42);
        assert_eq!(stats.mana, 69);
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities", "components"))]
    async fn test_storage_union(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = r#"
                { 
                    component_game: component(id: "component_1") {
                        name
                        storage {
                            __typename
                            ... on Game {
                                name
                                is_finished
                            }
                        }
                    }
                    component_stats: component(id: "component_2") {
                        name
                        storage {
                            __typename
                            ... on Stats {
                                health
                                mana
                            }
                        }
                    }
                }
            "#;
        let value = run_graphql_query(&pool, query).await;
        let component_game = value.get("component_game").ok_or("no component found").unwrap();
        let component_game: ComponentGame = serde_json::from_value(component_game.clone()).unwrap();
        let component_stats = value.get("component_stats").ok_or("no component found").unwrap();
        let component_stats: ComponentStats =
            serde_json::from_value(component_stats.clone()).unwrap();

        assert_eq!(component_game.name, component_game.storage.__typename);
        assert_eq!(component_stats.name, component_stats.storage.__typename);
    }
}
