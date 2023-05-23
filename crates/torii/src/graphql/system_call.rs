use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};

use super::system::{system_by_id, System};

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct SystemCall {
    pub id: i64,
    pub system_id: String,
    pub transaction_hash: String,
    pub data: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[ComplexObject]
impl SystemCall {
    async fn system(&self, context: &Context<'_>) -> Result<System> {
        let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
        system_by_id(&mut conn, self.system_id.clone()).await
    }
}

pub async fn system_call_by_id(conn: &mut PoolConnection<Sqlite>, id: i64) -> Result<SystemCall> {
    sqlx::query_as!(
        SystemCall,
        r#"
            SELECT
                id,
                data,
                transaction_hash,
                system_id,
                created_at as "created_at: _"
            FROM system_calls WHERE id = $1
        "#,
        id
    )
    .fetch_one(conn)
    .await
    .map_err(|err| err.into())
}

pub async fn system_calls_by_system(
    conn: &mut PoolConnection<Sqlite>,
    system_id: String,
) -> Result<Vec<SystemCall>> {
    sqlx::query_as!(
        SystemCall,
        r#"
            SELECT
                id,
                data,
                transaction_hash,
                system_id,
                created_at as "created_at: _"
            FROM system_calls WHERE system_id = $1
        "#,
        system_id
    )
    .fetch_all(conn)
    .await
    .map_err(|err| err.into())
}
