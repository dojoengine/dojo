use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};

use super::system_call::{system_calls_by_system, SystemCall};

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct System {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

#[ComplexObject]
impl System {
    async fn system_calls(&self, context: &Context<'_>) -> Result<Vec<SystemCall>> {
        let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
        system_calls_by_system(&mut conn, self.id.clone()).await
    }
}

pub async fn system_by_id(conn: &mut PoolConnection<Sqlite>, id: String) -> Result<System> {
    sqlx::query_as!(
        System,
        r#"
            SELECT
                id,
                name,
                address,
                class_hash,
                transaction_hash,
                created_at as "created_at: _"
            FROM systems WHERE id = $1
        "#,
        id
    )
    .fetch_one(conn)
    .await
    .map_err(|err| err.into())
}

pub async fn systems(conn: &mut PoolConnection<Sqlite>) -> Result<Vec<System>> {
    sqlx::query_as!(
        System,
        r#"
            SELECT
                id,
                name,
                address,
                class_hash,
                transaction_hash,
                created_at as "created_at: _"
            FROM systems
        "#
    )
    .fetch_all(conn)
    .await
    .map_err(|err| err.into())
}
