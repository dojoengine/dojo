use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};

use super::entity_state::{entity_states_by_component, EntityState};

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub properties: Option<String>,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

#[ComplexObject]
impl Component {
    async fn entity_states(&self, context: &Context<'_>) -> Result<Vec<EntityState>> {
        let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
        entity_states_by_component(&mut conn, self.id.clone()).await
    }
}

pub async fn component_by_id(conn: &mut PoolConnection<Sqlite>, id: String) -> Result<Component> {
    sqlx::query_as!(
        Component,
        r#"
            SELECT 
                id,
                name,
                properties,
                address,
                class_hash,
                transaction_hash,
                created_at as "created_at: _"
            FROM components WHERE id = $1
        "#,
        id
    )
    .fetch_one(conn)
    .await
    .map_err(|err| err.into())
}

pub async fn components(conn: &mut PoolConnection<Sqlite>) -> Result<Vec<Component>> {
    sqlx::query_as!(
        Component,
        r#"
            SELECT 
                id,
                name,
                properties,
                address,
                class_hash,
                transaction_hash,
                created_at as "created_at: _"
            FROM components
        "#
    )
    .fetch_all(conn)
    .await
    .map_err(|err| err.into())
}
