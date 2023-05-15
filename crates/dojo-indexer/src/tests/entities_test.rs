#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use crate::graphql::entity::Entity;
    use crate::tests::common::run_graphql_query;

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_entity(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ entity(id: \"entity_1\") { id name partitionId keys transactionHash \
                     createdAt updatedAt } }";
        let value = run_graphql_query(&pool, query).await;

        let entity = value.get("entity").ok_or("no entity found").unwrap();
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.id, "entity_1".to_string());
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_entities(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ entities { id name partitionId keys transactionHash createdAt updatedAt } }";
        let value = run_graphql_query(&pool, query).await;

        let entities = value.get("entities").ok_or("incorrect entities").unwrap();
        let entities: Vec<Entity> = serde_json::from_value(entities.clone()).unwrap();
        assert_eq!(entities.len(), 3);
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_entities_partition_id(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ entitiesByPartitionId (partitionId: \"420\") { id name partitionId keys \
                     transactionHash createdAt updatedAt } }";
        let value = run_graphql_query(&pool, query).await;

        let entities = value.get("entitiesByPartitionId").ok_or("incorrect entities").unwrap();
        let entities: Vec<Entity> = serde_json::from_value(entities.clone()).unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_entities_partition_id_keys(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ entityByPartitionIdKeys (partitionId: \"69\", keys: [\"420\"]) { id name \
                     partitionId keys transactionHash createdAt updatedAt } }";
        let value = run_graphql_query(&pool, query).await;

        let entity = value.get("entityByPartitionIdKeys").ok_or("no entity found").unwrap();
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.id, "entity_3".to_string());
    }
}
