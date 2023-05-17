use chrono::{DateTime, Utc};
use juniper::{graphql_object, FieldResult};
use serde::Deserialize;

use super::server::Context;
use super::system_call;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub keys: String,
    pub data: String,
    pub system_call_id: i64,
    pub created_at: DateTime<Utc>,
}

#[graphql_object(context = Context)]
impl Event {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn keys(&self) -> &str {
        &self.keys
    }

    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn system_call_id(&self) -> i32 {
        i32::try_from(self.system_call_id).unwrap()
    }

    pub async fn system_call(&self, context: &Context) -> FieldResult<system_call::SystemCall> {
        system_call::system_call(context, self.system_call_id).await
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
}

pub async fn event(context: &Context, id: String) -> FieldResult<Event> {
    let mut conn = context.pool.acquire().await?;

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
    .await?;

    Ok(event)
}

// flatten keys array and pattern match
pub async fn events(context: &Context, keys: Vec<String>) -> FieldResult<Vec<Event>> {
    let mut conn = context.pool.acquire().await?;
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
