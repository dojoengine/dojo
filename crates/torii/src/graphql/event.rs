use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{Pool, Sqlite};

use super::system_call;

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct Event {
    pub id: String,
    pub keys: String,
    pub data: String,
    pub system_call_id: i64,
    pub created_at: DateTime<Utc>,
}

#[ComplexObject]
impl Event {
    async fn system_call(&self, context: &Context<'_>) -> Result<system_call::SystemCall> {
        system_call::system_call(context, self.system_call_id).await
    }
}

pub async fn event(context: &Context<'_>, id: String) -> Result<Event> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let event = sqlx::query_as!(
        Event,
        r#"
            SELECT 
                id,
                system_call_id,
                keys,
                data,
                created_at as "created_at: _"
            FROM events 
            WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();

    Ok(event)
}

pub async fn events(context: &Context<'_>, keys: &[String]) -> Result<Vec<Event>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
    let keys_str = format!("{}%", keys.join(","));

    let events = sqlx::query_as!(
        Event,
        r#"
            SELECT 
                id,
                system_call_id,
                keys,
                data,
                created_at as "created_at: _"
            FROM events 
            WHERE keys LIKE ?
        "#,
        keys_str
    )
    .fetch_all(&mut conn)
    .await?;

    Ok(events)
}
