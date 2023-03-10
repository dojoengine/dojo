use juniper::{FieldResult, graphql_object};

use crate::server::Context;

use super::component;
use super::entity;

use entity::Entity;


pub struct EntityStateUpdate {
    pub id: i64,
    pub entity_id: String,
    pub component_id: String,
    pub transaction_hash: String,
    pub data: Option<String>,
}

#[graphql_object(context = Context)]
impl EntityStateUpdate {
    pub fn id(&self) -> i32 {
        i32::try_from(self.id).unwrap()
    }

    pub fn entity_id(&self) -> &str {
        &self.entity_id
    }

    pub fn component_id(&self) -> &str {
        &self.component_id
    }

    pub fn transaction_hash(&self) -> &str {
        &self.transaction_hash
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

pub async fn entity_state_update(context: &Context, id: i64) -> FieldResult<EntityStateUpdate> {
    let mut conn = context.pool.acquire().await.unwrap();
    
    let entity_state_update = sqlx::query_as!(
        EntityStateUpdate,
        r#"
            SELECT * FROM entity_state_updates WHERE id = $1
        "#,
        id
    ).fetch_one(&mut conn).await.unwrap();

    Ok(entity_state_update)
}

pub async fn entity_state_updates(context: &Context) -> FieldResult<Vec<EntityStateUpdate>> {
    let mut conn = context.pool.acquire().await.unwrap();
    
    let entity_state_updates = sqlx::query_as!(
        EntityStateUpdate,
        r#"
            SELECT * FROM entity_state_updates
        "#
    ).fetch_all(&mut conn).await.unwrap();

    Ok(entity_state_updates)
}

pub async fn entity_state_updates_by_component(context: &Context, component_id: String) -> FieldResult<Vec<EntityStateUpdate>> {
    let mut conn = context.pool.acquire().await.unwrap();
    
    let entity_state_updates = sqlx::query_as!(
        EntityStateUpdate,
        r#"
            SELECT * FROM entity_state_updates WHERE component_id = $1
        "#,
        component_id
    ).fetch_all(&mut conn).await.unwrap();

    Ok(entity_state_updates)
}

pub async fn entity_state_updates_by_entity(context: &Context, entity_id: String) -> FieldResult<Vec<EntityStateUpdate>> {
    let mut conn = context.pool.acquire().await.unwrap();
    
    let entity_state_updates = sqlx::query_as!(
        EntityStateUpdate,
        r#"
            SELECT * FROM entity_state_updates WHERE entity_id = $1
        "#,
        entity_id
    ).fetch_all(&mut conn).await.unwrap();

    Ok(entity_state_updates)
}

// Copy the content of entity

