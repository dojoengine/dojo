use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{Pool, Sqlite};

use super::entity_state;

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
    async fn entity_states(&self, context: &Context<'_>) -> Result<Vec<entity_state::EntityState>> {
        entity_state::entity_states_by_component(context, self.id.clone()).await
    }
}

pub async fn component(context: &Context<'_>, id: String) -> Result<Component> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

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

pub async fn components(context: &Context<'_>) -> Result<Vec<Component>> {
    let mut conn = context.data::<Pool<Sqlite>>()?.acquire().await?;

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
