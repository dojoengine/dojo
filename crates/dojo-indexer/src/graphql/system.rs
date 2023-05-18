use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{Pool, Sqlite};

use super::system_call;

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct System {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

#[ComplexObject]
impl System {
    async fn system_calls(&self, context: &Context<'_>) -> Result<Vec<system_call::SystemCall>> {
        system_call::system_calls_by_system(context, self.id.clone()).await
    }
}

pub async fn system(context: &Context<'_>, id: String) -> Result<System> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let system = sqlx::query_as!(
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
    .fetch_one(&mut conn)
    .await?;

    Ok(system)
}

pub async fn systems(context: &Context<'_>) -> Result<Vec<System>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let systems = sqlx::query_as!(
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
    .fetch_all(&mut conn)
    .await?;

    Ok(systems)
}
