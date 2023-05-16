use entity::Entity;
use juniper::{graphql_object, FieldResult};
use chrono::{DateTime, Utc};

use super::server::Context;
use super::{component, entity};

pub struct EntityState {
    pub entity_id: String,
    pub component_id: String,
    pub data: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[graphql_object(context = Context)]
impl EntityState {
    pub fn entity_id(&self) -> &str {
        &self.entity_id
    }

    pub fn component_id(&self) -> &str {
        &self.component_id
    }

    pub fn data(&self) -> &Option<String> {
        &self.data
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    async fn component(&self, context: &Context) -> FieldResult<component::Component> {
        component::component(context, self.component_id.clone()).await
    }

    async fn entity(context: &Context) -> FieldResult<Entity> {
        entity::entity(context, self.entity_id.clone()).await
    }
}

pub async fn entity_states_by_entity(
    context: &Context,
    entity_id: String,
) -> FieldResult<Vec<EntityState>> {
    let mut conn = context.pool.acquire().await.unwrap();

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
    context: &Context,
    component_id: String,
) -> FieldResult<Vec<EntityState>> {
    let mut conn = context.pool.acquire().await.unwrap();

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
