use chrono::{DateTime, Utc};
use juniper::{graphql_object, FieldResult};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::Sqlite;

use super::entity_state::EntityState;
use super::entity_state_update::EntityStateUpdate;
use super::server::Context;
use super::{entity_state, entity_state_update};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub name: Option<String>,
    pub partition_id: String,
    pub keys: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[graphql_object(context = Context)]
impl Entity {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn partition_id(&self) -> &str {
        &self.partition_id
    }

    pub fn keys(&self) -> &str {
        &self.keys
    }

    pub fn transaction_hash(&self) -> &str {
        &self.transaction_hash
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub async fn state_updates(&self, context: &Context) -> FieldResult<Vec<EntityStateUpdate>> {
        entity_state_update::entity_state_updates_by_entity(context, self.id.clone()).await
    }

    pub async fn states(&self, context: &Context) -> FieldResult<Vec<EntityState>> {
        entity_state::entity_states_by_entity(context, self.id.clone()).await
    }
}

pub async fn entity(context: &Context, id: String) -> FieldResult<Entity> {
    let mut conn = context.pool.acquire().await?;

    // timestamp workaround: https://github.com/launchbadge/sqlx/issues/598
    let entity = sqlx::query_as!(
        Entity,
        r#"
            SELECT 
                id,
                name,
                partition_id,
                keys,
                transaction_hash,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM entities 
            WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(entity)
}

pub async fn entities(
    context: &Context,
    partition_id: String,
    keys: Option<Vec<String>>,
) -> FieldResult<Vec<Entity>> {
    let mut conn = context.pool.acquire().await?;

    match keys {
        Some(keys) => query_by_keys(&mut conn, partition_id, keys).await,
        None => query_by_partition(&mut conn, partition_id).await,
    }
}

async fn query_by_keys(
    conn: &mut PoolConnection<Sqlite>,
    partition_id: String,
    keys: Vec<String>,
) -> FieldResult<Vec<Entity>> {
    let keys_str = format!("{}%", keys.join(","));
    let entities = sqlx::query_as!(
        Entity,
        r#"
            SELECT                 
                id,
                name,
                partition_id,
                keys,
                transaction_hash,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM entities where partition_id = $1 AND keys LIKE $2
        "#,
        partition_id,
        keys_str
    )
    .fetch_all(conn)
    .await?;

    Ok(entities)
}

async fn query_by_partition(
    conn: &mut PoolConnection<Sqlite>,
    partition_id: String,
) -> FieldResult<Vec<Entity>> {
    let entities = sqlx::query_as!(
        Entity,
        r#"
            SELECT 
                id,
                name,
                partition_id,
                keys,
                transaction_hash,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM entities where partition_id = $1
        "#,
        partition_id,
    )
    .fetch_all(conn)
    .await?;

    Ok(entities)
}
