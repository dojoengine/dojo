use async_graphql::connection::{query, Connection, Edge, OpaqueCursor};
use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject, ID};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, QueryBuilder, Sqlite};

use super::constants::DEFAULT_LIMIT;
use super::entity_state::{entity_states_by_entity, EntityState};

#[derive(SimpleObject, Debug, Deserialize, FromRow)]
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
    async fn states(&self, context: &Context<'_>) -> Result<Vec<EntityState>> {
        let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
        entity_states_by_entity(&mut conn, self.id.clone()).await
    }
}

pub async fn entities_by_pk(
    conn: &mut PoolConnection<Sqlite>,
    partition_id: String,
    keys: Option<Vec<String>>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> Result<Connection<OpaqueCursor<ID>, Entity>> {
    query(
        after,
        before,
        first,
        last,
        | after: Option<OpaqueCursor<ID>>,
            before: Option<OpaqueCursor<ID>>,
            first,
            last| async move {

        let mut builder: QueryBuilder<'_, Sqlite>  = QueryBuilder::new("SELECT * FROM entities");
        builder.push(" WHERE partition_id = ")
            .push_bind(partition_id);

        if let Some(keys) = keys {
            let keys_str = format!("{}%", keys.join(","));
            builder.push(" AND keys LIKE ")
                .push_bind(keys_str);
        }

        if let Some(after) = after {
            let entity = entity_by_id(conn, after.0.to_string()).await?;
            let created_at = entity.created_at.to_rfc3339_opts(SecondsFormat::Secs, true);
            builder.push(" AND created_at > ")
                .push_bind(created_at);
        }

        if let Some(before) = before {
            let entity = entity_by_id(conn, before.0.to_string()).await?;
            let created_at = entity.created_at.to_rfc3339_opts(SecondsFormat::Secs, true);
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

        let entities: Vec<Entity> = builder.build_query_as().fetch_all(conn).await?;

        // TODO: hasPreviousPage, hasNextPage
        let mut connection = Connection::new(true, true);
        for entity in entities {
            connection.edges.push(
                Edge::new(
                    OpaqueCursor(ID(entity.id.clone())),
                    entity
                )
            );
        }
        Ok::<_, Error>(connection)
    }).await
}

pub async fn entity_by_id(conn: &mut PoolConnection<Sqlite>, id: String) -> Result<Entity> {
    // timestamp workaround: https://github.com/launchbadge/sqlx/issues/598
    sqlx::query_as!(
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
        id,
    )
    .fetch_one(conn)
    .await
    .map_err(|err| err.into())
}

async fn _entities_by_keys(
    conn: &mut PoolConnection<Sqlite>,
    partition_id: String,
    keys: &[String],
) -> Result<Vec<Entity>> {
    let keys_str = format!("{}%", keys.join(","));
    sqlx::query_as!(
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
    .await
    .map_err(|err| err.into())
}

async fn _entities_by_partition(
    conn: &mut PoolConnection<Sqlite>,
    partition_id: String,
) -> Result<Vec<Entity>> {
    sqlx::query_as!(
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
    .await
    .map_err(|err| err.into())
}
