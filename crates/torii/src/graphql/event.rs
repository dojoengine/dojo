use async_graphql::connection::{query, Connection, Edge, OpaqueCursor};
use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject, ID};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::query_builder::QueryBuilder;
use sqlx::{FromRow, Pool, Sqlite};

use super::constants::DEFAULT_LIMIT;
use super::system_call::{system_call_by_id, SystemCall};

#[derive(FromRow, SimpleObject, Debug, Deserialize)]
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
    async fn system_call(&self, context: &Context<'_>) -> Result<SystemCall> {
        let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
        system_call_by_id(&mut conn, self.system_call_id).await
    }
}

pub async fn events_by_keys(
    conn: &mut PoolConnection<Sqlite>,
    keys: &[String],
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> Result<Connection<OpaqueCursor<ID>, Event>> {
    query(
        after,
        before,
        first,
        last,
        | after: Option<OpaqueCursor<ID>>,
            before: Option<OpaqueCursor<ID>>,
            first,
            last| async move {

        let keys_str = format!("{}%", keys.join(","));

        let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM events");
        builder.push(" WHERE keys LIKE ")
            .push_bind(keys_str.as_str());

        if let Some(after) = after {
            let event = event_by_id(conn, after.0.to_string()).await?;
            let created_at = event.created_at.to_rfc3339_opts(SecondsFormat::Secs, true);
            builder.push(" AND created_at > ")
                .push_bind(created_at);
        }

        if let Some(before) = before {
            let event = event_by_id(conn, before.0.to_string()).await?;
            let created_at = event.created_at.to_rfc3339_opts(SecondsFormat::Secs, true);
            builder.push(" AND created_at < ")
                .push_bind(created_at);
        }

        let order = match last {
            Some(_) => "ASC",
            None => "DESC",
        };
        builder.push(" ORDER BY created_at ").push(order);

        let limit = match first.or(last) {
            Some(limit) => limit,
            None => DEFAULT_LIMIT,
        };
        builder.push(" LIMIT ").push(limit.to_string());

        let events: Vec<Event> = builder.build_query_as().fetch_all(conn).await?;

        // TODO: hasPreviousPage, hasNextPage
        let mut connection = Connection::new(true, true);
        for event in events {
            connection.edges.push(
                Edge::new(
                    OpaqueCursor(ID(event.id.clone())),
                    event
                ));
        }
        Ok::<_, Error>(connection)
    }).await
}

pub async fn event_by_id(conn: &mut PoolConnection<Sqlite>, id: String) -> Result<Event> {
    sqlx::query_as!(
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
    .fetch_one(conn)
    .await
    .map_err(|err| err.into())
}
