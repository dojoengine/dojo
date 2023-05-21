use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};

use super::component::{component_by_id, Component};
use super::entity::{entity_by_id, Entity};

#[derive(SimpleObject, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct EntityState {
    pub entity_id: String,
    pub component_id: String,
    pub data: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[ComplexObject]
impl EntityState {
    async fn entity(&self, context: &Context<'_>) -> Result<Entity> {
        let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
        entity_by_id(&mut conn, self.entity_id.clone()).await
    }

    async fn component(&self, context: &Context<'_>) -> Result<Component> {
        let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;
        component_by_id(&mut conn, self.component_id.clone()).await
    }
}

pub async fn entity_states_by_entity(
    conn: &mut PoolConnection<Sqlite>,
    entity_id: String,
) -> Result<Vec<EntityState>> {
    sqlx::query_as!(
        EntityState,
        r#"
            SELECT
                entity_id,
                component_id,
                data,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM entity_states WHERE entity_id = $1
        "#,
        entity_id
    )
    .fetch_all(conn)
    .await
    .map_err(|err| err.into())
}

pub async fn entity_states_by_component(
    conn: &mut PoolConnection<Sqlite>,
    component_id: String,
) -> Result<Vec<EntityState>> {
    sqlx::query_as!(
        EntityState,
        r#"
            SELECT
                entity_id,
                component_id,
                data,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM entity_states WHERE component_id = $1
        "#,
        component_id
    )
    .fetch_all(conn)
    .await
    .map_err(|err| err.into())
}
