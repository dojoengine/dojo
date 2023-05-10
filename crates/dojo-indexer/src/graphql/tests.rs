use std::sync::Arc;
use serde_json::Value;
use sqlx::{Sqlite, SqlitePool};
use sqlx::pool::PoolConnection;
use chrono::Utc;
use juniper::{EmptyMutation, EmptySubscription, Variables};

use super::component::Component;
use super::entity::Entity;
use super::system::System;

use super::server::Context;
use super::server::Schema;
use super::Query;

async fn insert_sqlx_data(mut conn: PoolConnection<Sqlite>) -> sqlx::Result<()> {
    for i in 0..3 {
        let entity = Entity {
            id: i.to_string(),
            name: Some("test".to_string()),
            partition_id: "420".to_string(),
            partition_keys: (i*2).to_string(),
            transaction_hash: "0x0".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let _ = sqlx::query_as!(
            Entity,
            r#"
                INSERT INTO entities (
                    id, 
                    name, 
                    partition_id, 
                    partition_keys, 
                    transaction_hash, 
                    created_at,
                    updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            entity.id,
            entity.name,
            entity.partition_id,
            entity.partition_keys,
            entity.transaction_hash,
            entity.created_at,
            entity.updated_at
        ).execute(&mut conn).await?;
    }
    Ok(())
}


async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
    let context = Context {
        schema: Arc::new(
            Schema::new(
                Query,
                EmptyMutation::<Context>::new(),
                EmptySubscription::<Context>::new(),
            ),
        ),
        pool: Arc::new(pool.clone()),
    };
    let schema = context.schema.clone();
    let (result, error) =
        juniper::execute(query, None, &schema, &Variables::new(), &context)
            .await
            .unwrap();
    assert!(error.is_empty());

    serde_json::from_str(&(result.to_string())).unwrap()
}


#[sqlx::test(migrations = "./migrations")]
async fn test_entity(pool: SqlitePool) -> sqlx::Result<()> {
    let conn = pool.acquire().await?;
    insert_sqlx_data(conn).await?;

    let query = "{ entity(id: \"1\") { id name partitionId partitionKeys transactionHash createdAt updatedAt } }";
    let value = run_graphql_query(&pool, query).await;

    if let Some(entity) = value.get("entity") {
        let entity: Entity = serde_json::from_value(entity.clone()).unwrap();
        assert_eq!(entity.id, "1".to_string());
        return Ok(());
    }
    panic!("no entity found");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_entities(pool: SqlitePool) -> sqlx::Result<()> {
    let conn = pool.acquire().await?;
    insert_sqlx_data(conn).await?;

    let query = "{ entities { id name partitionId partitionKeys transactionHash createdAt updatedAt } }";
    let value = run_graphql_query(&pool, query).await;

    if let Some(entities) = value.get("entities") {
        let entities: Vec<Entity> = serde_json::from_value(entities.clone()).unwrap();
        assert_eq!(entities.len(), 3);
        return Ok(());
    }
    panic!("incorrect entities");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_entities_partition_id(pool: SqlitePool) -> sqlx::Result<()> {
    let conn = pool.acquire().await?;
    insert_sqlx_data(conn).await?;

    let query = "{ entities(partitionId: \"420\") { id name partitionId partitionKeys transactionHash createdAt updatedAt } }";
    let value = run_graphql_query(&pool, query).await;

    if let Some(entities) = value.get("entities") {
        let entities: Vec<Entity> = serde_json::from_value(entities.clone()).unwrap();
        assert_eq!(entities.len(), 3);
        return Ok(());
    }
    panic!("incorrect entities");
}