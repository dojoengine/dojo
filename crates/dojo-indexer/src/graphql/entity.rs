use juniper::{graphql_object, FieldResult};

use super::entity_state::EntityState;
use super::entity_state_update::EntityStateUpdate;
use super::server::Context;
use super::{entity_state, entity_state_update};

pub struct Entity {
    pub id: String,
    pub name: Option<String>,
    pub transaction_hash: String,
}

#[graphql_object(context = Context)]
impl Entity {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn transaction_hash(&self) -> &str {
        &self.transaction_hash
    }

    pub async fn state_updates(&self, context: &Context) -> FieldResult<Vec<EntityStateUpdate>> {
        entity_state_update::entity_state_updates_by_entity(context, self.id.clone()).await
    }

    pub async fn states(&self, context: &Context) -> FieldResult<Vec<EntityState>> {
        entity_state::entity_states_by_entity(context, self.id.clone()).await
    }
}

pub async fn entity(context: &Context, id: String) -> FieldResult<Entity> {
    let mut conn = context.pool.acquire().await.unwrap();

    let entity = sqlx::query_as!(
        Entity,
        r#"
            SELECT * FROM entities WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(entity)
}

pub async fn entities(context: &Context) -> FieldResult<Vec<Entity>> {
    let mut conn = context.pool.acquire().await.unwrap();

    let entities = sqlx::query_as!(
        Entity,
        r#"
            SELECT * FROM entities
        "#
    )
    .fetch_all(&mut conn)
    .await?;

    Ok(entities)
}
