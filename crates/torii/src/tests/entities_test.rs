#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use sqlx::SqlitePool;

    use crate::tests::common::run_graphql_query;

    #[derive(Deserialize)]
    pub struct Entity {
        pub id: String,
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_entity(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ entity(id: \"entity_1\") { id } }";
        let value = run_graphql_query(&pool, query).await;

        let entity = value.get("entity").ok_or("no entity found").unwrap();
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.id, "entity_1".to_string());
    }

    // #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    // async fn test_entities_partition_id(pool: SqlitePool) {
    //     let _ = pool.acquire().await;

    //     let query = "{ entities (partitionId: \"420\") { edges { node { id name partitionId keys
    // \                  transactionHash createdAt } } } }";
    //     let value = run_graphql_query(&pool, query).await;

    //     let entities = value.get("entities").ok_or("incorrect entities").unwrap();
    //     let edges = entities.get("edges").ok_or("incorrect edges").unwrap();
    //     let edges: Vec<serde_json::Value> = serde_json::from_value(edges.clone()).unwrap();
    //     assert_eq!(edges.len(), 2);
    // }

    // #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    // async fn test_entities_partition_id_keys(pool: SqlitePool) {
    //     let _ = pool.acquire().await;

    //     let query = "{ entities (partitionId: \"69\", keys: [\"420\"]) { edges { node { id name \
    //                  partitionId keys transactionHash createdAt } } } }";
    //     let value = run_graphql_query(&pool, query).await;

    //     let entities = value.get("entities").ok_or("incorrect entities").unwrap();
    //     let edges = entities.get("edges").ok_or("incorrect edges").unwrap();
    //     let node = edges[0].get("node").ok_or("incorrect node").unwrap();
    //     let entity: Entity = serde_json::from_value(node.clone()).unwrap();
    //     assert_eq!(entity.id, "entity_3".to_string());
    // }
}
