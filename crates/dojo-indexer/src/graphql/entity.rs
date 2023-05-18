use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};

use super::entity_state;

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub partition_id: String,
    pub keys: Option<String>,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

#[ComplexObject]
impl Entity {
    async fn states(&self, context: &Context<'_>) -> Result<Vec<entity_state::EntityState>> {
        entity_state::entity_states_by_entity(context, self.id.clone()).await
    }
}

pub async fn entity(context: &Context<'_>, id: String) -> Result<Entity> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

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
                created_at as "created_at: _"
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
    context: &Context<'_>,
    partition_id: String,
    keys: Option<Vec<String>>,
) -> Result<Vec<Entity>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    match keys {
        Some(keys) => query_by_keys(&mut conn, partition_id, keys).await,
        None => query_by_partition(&mut conn, partition_id).await,
    }
}

async fn query_by_keys(
    conn: &mut PoolConnection<Sqlite>,
    partition_id: String,
    keys: Vec<String>,
) -> Result<Vec<Entity>> {
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
                created_at as "created_at: _"
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
) -> Result<Vec<Entity>> {
    let entities = sqlx::query_as!(
        Entity,
        r#"
            SELECT 
                id,
                name,
                partition_id,
                keys,
                transaction_hash,
                created_at as "created_at: _"
            FROM entities where partition_id = $1
        "#,
        partition_id,
    )
    .fetch_all(conn)
    .await?;

    Ok(entities)
}
