#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use crate::graphql::event::Event;
    use crate::tests::common::run_graphql_query;

    #[sqlx::test(migrations = "./migrations", fixtures("systems", "system_calls", "events"))]
    async fn test_event(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ event(id: \"event_1\") { id keys data systemCallId createdAt } }";
        let value = run_graphql_query(&pool, query).await;

        let event = value.get("event").ok_or("no event found").unwrap();
        let event: Event = serde_json::from_value(event.clone()).unwrap();
        assert_eq!(event.id, "event_1".to_string());
    }

    #[sqlx::test(migrations = "./migrations", fixtures("systems", "system_calls", "events"))]
    async fn test_event_by_keys(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ events(keys: [\"key_1\", \"key_2\", \"key_3\"]) { edges { node { id keys \
                     data systemCallId createdAt } } } }";
        let value = run_graphql_query(&pool, query).await;
        let events = value.get("events").ok_or("no event found").unwrap();
        let edges = events.get("edges").ok_or("no event found").unwrap();
        let edges: Vec<serde_json::Value> = serde_json::from_value(edges.clone()).unwrap();
        let node = edges[0].get("node").ok_or("no event found").unwrap();
        let event: Event = serde_json::from_value(node.clone()).unwrap();
        assert_eq!(event.id, "event_1".to_string());

        let query = "{ events(keys: [\"key_1\", \"key_2\"]) { edges { node { id keys data \
                     systemCallId createdAt } } } }";
        let value = run_graphql_query(&pool, query).await;
        let events = value.get("events").ok_or("no event found").unwrap();
        let edges = events.get("edges").ok_or("no event found").unwrap();
        let edges: Vec<serde_json::Value> = serde_json::from_value(edges.clone()).unwrap();
        assert_eq!(edges.len(), 2);

        let query = "{ events(keys: [\"key_3\"]) { edges { node { id keys data systemCallId \
                     createdAt } } } }";
        let value = run_graphql_query(&pool, query).await;
        let events = value.get("events").ok_or("no event found").unwrap();
        let edges = events.get("edges").ok_or("no event found").unwrap();
        let edges: Vec<serde_json::Value> = serde_json::from_value(edges.clone()).unwrap();
        let node = edges[0].get("node").ok_or("no event found").unwrap();
        let event: Event = serde_json::from_value(node.clone()).unwrap();
        assert_eq!(event.id, "event_3".to_string());
    }
}
