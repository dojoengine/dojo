// Copy entity_state_update to entity_state.rs and replace all entity_state_update with entity_state
//
// Copy the content of entity

use juniper::{FieldResult, graphql_object};

use crate::server::Context;

use super::component;
use super::entity;

use entity::Entity;

pub struct EntityState {
    pub entity_id: String,
    pub component_id: String,
    pub data: Option<String>,
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

    async fn component(&self, context: &Context) -> FieldResult<component::Component> {
        component::component(context, self.component_id.clone()).await
    }

    async fn entity(context: &Context) -> FieldResult<Entity> {
        entity::entity(context, self.entity_id.clone()).await
    }
}

pub async fn entity_states(context: &Context) -> FieldResult<Vec<EntityState>> {
    let mut conn = context.pool.acquire().await.unwrap();
    
    let entity_states = sqlx::query_as!(
        EntityState,
        r#"
            SELECT * FROM entity_states
        "#
    ).fetch_all(&mut conn).await.unwrap();

    Ok(entity_states)
}

pub async fn entity_states_by_entity(context: &Context, entity_id: String) -> FieldResult<Vec<EntityState>> {
    let mut conn = context.pool.acquire().await.unwrap();
    
    let entity_states = sqlx::query_as!(
        EntityState,
        r#"
            SELECT * FROM entity_states WHERE entity_id = $1
        "#,
        entity_id
    ).fetch_all(&mut conn).await.unwrap();

    Ok(entity_states)
}

pub async fn entity_states_by_component(context: &Context, component_id: String) -> FieldResult<Vec<EntityState>> {
    let mut conn = context.pool.acquire().await.unwrap();
    
    let entity_states = sqlx::query_as!(
        EntityState,
        r#"
            SELECT * FROM entity_states WHERE component_id = $1
        "#,
        component_id
    ).fetch_all(&mut conn).await.unwrap();

    Ok(entity_states)
}