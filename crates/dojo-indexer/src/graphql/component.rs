use juniper::{graphql_object, FieldResult};
use chrono::{DateTime, Utc};

use super::server::Context;

pub struct Component {
    pub id: String,
    pub name: Option<String>,
    pub properties: Option<String>,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

#[graphql_object(context = Context)]
impl Component {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn properties(&self) -> &Option<String> {
        &self.properties
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn class_hash(&self) -> &str {
        &self.class_hash
    }

    pub fn transaction_hash(&self) -> &str {
        &self.transaction_hash
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }


    pub async fn entity_states(
        &self,
        context: &Context,
    ) -> FieldResult<Vec<super::entity_state::EntityState>> {
        super::entity_state::entity_states_by_component(context, self.id.clone()).await
    }
}

pub async fn component(context: &Context, id: String) -> FieldResult<Component> {
    let mut conn = context.pool.acquire().await.unwrap();

    let component = sqlx::query_as!(
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
    .fetch_one(&mut conn)
    .await?;

    Ok(component)
}

pub async fn components(context: &Context) -> FieldResult<Vec<Component>> {
    let mut conn = context.pool.acquire().await.unwrap();

    let components = sqlx::query_as!(
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
    .fetch_all(&mut conn)
    .await?;

    Ok(components)
}
