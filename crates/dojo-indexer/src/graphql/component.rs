use juniper::{graphql_object, FieldResult};

use super::server::Context;

pub struct Component {
    pub id: String,
    pub name: Option<String>,
    pub properties: Option<String>,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
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

    pub async fn entity_state_updates(
        &self,
        context: &Context,
    ) -> FieldResult<Vec<super::entity_state_update::EntityStateUpdate>> {
        super::entity_state_update::entity_state_updates_by_component(context, self.id.clone())
            .await
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
            SELECT * FROM components WHERE id = $1
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
            SELECT * FROM components
        "#
    )
    .fetch_all(&mut conn)
    .await?;

    Ok(components)
}
