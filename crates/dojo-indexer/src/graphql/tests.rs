use std::sync::Arc;

use juniper::{EmptyMutation, EmptySubscription, Variables};
use serde_json::Value;
use sqlx::SqlitePool;

use super::entity::Entity;
use super::server::{Context, Schema};
use super::Query;

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
    let (result, error) =
        juniper::execute(query, None, &schema, &Variables::new(), &context).await.unwrap();
    assert!(error.is_empty());

    serde_json::from_str(&(result.to_string())).unwrap()
}

#[sqlx::test(migrations = "./migrations", fixtures("entities"))]
async fn test_entity(pool: SqlitePool) {
    let _ = pool.acquire().await;

    let query = "{ entity(id: \"1\") { id name partitionId partitionKeys transactionHash \
                 createdAt updatedAt } }";
    let value = run_graphql_query(&pool, query).await;

    let entity = value.get("entity").ok_or_else(|| "no entity found").unwrap();
    let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
    assert_eq!(entity.id, "1".to_string());
}

#[sqlx::test(migrations = "./migrations", fixtures("entities"))]
async fn test_entities(pool: SqlitePool) {
    let _ = pool.acquire().await;

    let query =
        "{ entities { id name partitionId partitionKeys transactionHash createdAt updatedAt } }";
    let value = run_graphql_query(&pool, query).await;

    let entities = value.get("entities").ok_or_else(|| "incorrect entities").unwrap();
    let entities: Vec<Entity> = serde_json::from_value(entities.clone()).unwrap();
    assert_eq!(entities.len(), 3);
}

#[sqlx::test(migrations = "./migrations", fixtures("entities"))]
async fn test_entities_partition_id(pool: SqlitePool) {
    let _ = pool.acquire().await;

    let query = "{ entitiesByPartitionId (partitionId: \"420\") { id name partitionId \
                 partitionKeys transactionHash createdAt updatedAt } }";
    let value = run_graphql_query(&pool, query).await;

    let entities = value.get("entitiesByPartitionId").ok_or_else(|| "incorrect entities").unwrap();
    let entities: Vec<Entity> = serde_json::from_value(entities.clone()).unwrap();
    assert_eq!(entities.len(), 2);
}

#[sqlx::test(migrations = "./migrations", fixtures("entities"))]
async fn test_entities_partition_id_keys(pool: SqlitePool) {
    let _ = pool.acquire().await;

    let query = "{ entityByPartitionIdKeys (partitionId: \"69\", partitionKeys: \"420\") { id \
                 name partitionId partitionKeys transactionHash createdAt updatedAt } }";
    let value = run_graphql_query(&pool, query).await;

    let entity = value.get("entityByPartitionIdKeys").ok_or_else(|| "no entity found").unwrap();
    let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
    assert_eq!(entity.id, "3".to_string());
}
