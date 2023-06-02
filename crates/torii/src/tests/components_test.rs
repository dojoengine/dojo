#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use sqlx::SqlitePool;

    use crate::tests::common::run_graphql_query;

    #[derive(Deserialize)]
    struct Game {
        name: String,
        is_finished: bool,
    }

    #[derive(Deserialize)]
    struct Stats {
        health: i64,
        mana: i64,
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities", "components"))]
    async fn test_storage_components(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ game(id: 1) { name is_finished } stats(id: 1) { health mana } }";
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
}
