use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{Pool, Sqlite};

use super::{component, entity};

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
    async fn entity(&self, context: &Context<'_>) -> Result<entity::Entity> {
        entity::entity(context, self.entity_id.clone()).await
    }

    async fn component(&self, context: &Context<'_>) -> Result<component::Component> {
        component::component(context, self.component_id.clone()).await
    }
}

pub async fn entity_states_by_entity(
    context: &Context<'_>,
    entity_id: String,
) -> Result<Vec<EntityState>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let entity_states = sqlx::query_as!(
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
    .fetch_all(&mut conn)
    .await?;

    Ok(entity_states)
}

pub async fn entity_states_by_component(
    context: &Context<'_>,
    component_id: String,
) -> Result<Vec<EntityState>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

    let entity_states = sqlx::query_as!(
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
    .fetch_all(&mut conn)
    .await?;

    Ok(entity_states)
}
