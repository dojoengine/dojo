#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use juniper::{EmptyMutation, EmptySubscription, Variables};
    use serde_json::Value;
    use sqlx::SqlitePool;

    use crate::graphql::entity::Entity;
    use crate::graphql::server::{Context, Schema};
    use crate::graphql::Query;

    async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
        let context = Context {
            schema: Arc::new(Schema::new(
                Query,
                EmptyMutation::<Context>::new(),
                EmptySubscription::<Context>::new(),
            )),
            pool: Arc::new(pool.clone()),
        };

        let schema = context.schema.clone();
        let (result, error) = juniper::execute(query, None, &schema, &Variables::new(), &context)
            .await
            .unwrap_or_else(|error| panic!("GraphQL query failed: {}", error));

        assert!(error.is_empty(), "GraphQL query returned errors: {:?}", error);

        serde_json::from_str(&result.to_string())
            .expect("Failed to parse GraphQL query result as JSON")
    }

    #[sqlx::test(migrations = "./migrations", fixtures("entities"))]
    async fn test_entity(pool: SqlitePool) {
        let _ = pool.acquire().await;

        let query = "{ entity(id: \"1\") { id name partitionId keys transactionHash createdAt \
                     updatedAt } }";
        let value = run_graphql_query(&pool, query).await;

        let entity = value.get("entity").ok_or("no entity found").unwrap();
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.id, "1".to_string());
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

        let query = "{ entityByPartitionIdKeys (partitionId: \"69\", keys: \"420\") { id name \
                     partitionId keys transactionHash createdAt updatedAt } }";
        let value = run_graphql_query(&pool, query).await;

        let entity = value.get("entityByPartitionIdKeys").ok_or("no entity found").unwrap();
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.id, "3".to_string());
    }
}
