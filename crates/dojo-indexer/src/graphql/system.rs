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
}

#[ComplexObject]
impl System {
    async fn system_calls<'ctx>(
        &self,
        context: &Context<'ctx>,
    ) -> Result<Vec<system_call::SystemCall>> {
        system_call::system_calls_by_system(context, self.id.clone()).await
    }
}

pub async fn system<'ctx>(context: &Context<'ctx>, id: String) -> Result<System> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let system = sqlx::query_as!(
        System,
        r#"
            SELECT * FROM systems WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(system)
}

pub async fn systems<'ctx>(context: &Context<'ctx>) -> Result<Vec<System>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let systems = sqlx::query_as!(
        System,
        r#"
            SELECT * FROM systems
        "#
    )
    .fetch_all(&mut conn)
    .await?;

    Ok(systems)
}
