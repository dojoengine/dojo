use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{Pool, Sqlite};

use super::system;

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct SystemCall {
    pub id: i64,
    pub system_id: String,
    pub transaction_hash: String,
    pub data: Option<String>,
}

#[ComplexObject]
impl SystemCall {
    async fn system<'ctx>(&self, context: &Context<'ctx>) -> Result<system::System> {
        system::system(&context, self.system_id.clone()).await
    }
}

pub async fn system_call<'ctx>(context: &Context<'ctx>, id: i64) -> Result<SystemCall> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let system_call = sqlx::query_as!(
        SystemCall,
        r#"
            SELECT * FROM system_calls WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(system_call)
}

pub async fn system_calls_by_system<'ctx>(
    context: &Context<'ctx>,
    system_id: String,
) -> Result<Vec<SystemCall>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let system_calls = sqlx::query_as!(
        SystemCall,
        r#"
            SELECT * FROM system_calls WHERE system_id = $1
        "#,
        system_id
    )
    .fetch_all(&mut conn)
    .await?;

    Ok(system_calls)
}
