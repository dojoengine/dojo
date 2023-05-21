pub mod component;
pub mod constants;
pub mod entity;
pub mod entity_state;
pub mod event;
pub mod server;
pub mod system;
pub mod system_call;

use async_graphql::connection::{Connection, OpaqueCursor};
use async_graphql::{Context, Object, Result, ID};
use component::{component_by_id, Component};
use entity::{entities_by_pk, entity_by_id, Entity};
use event::{event_by_id, events_by_keys, Event};
use sqlx::{Pool, Sqlite};
use system::{system_by_id, System};

pub struct Query;

#[Object]
impl Query {
    async fn component(&self, ctx: &Context<'_>, id: ID) -> Result<Component> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
        component_by_id(&mut conn, id.0.to_string()).await
    }

    async fn components(&self, ctx: &Context<'_>) -> Result<Vec<Component>> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;

        // TODO: handle pagination
        component::components(&mut conn).await
    }

    async fn system(&self, ctx: &Context<'_>, id: ID) -> Result<System> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
        system_by_id(&mut conn, id.0.to_string()).await
    }

    async fn systems(&self, ctx: &Context<'_>) -> Result<Vec<System>> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;

        // TODO: handle pagination
        system::systems(&mut conn).await
    }

    async fn entity(&self, ctx: &Context<'_>, id: ID) -> Result<Entity> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
        entity_by_id(&mut conn, id.0.to_string()).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn entities(
        &self,
        ctx: &Context<'_>,
        partition_id: String,
        keys: Option<Vec<String>>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<OpaqueCursor<ID>, Entity>> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
        entities_by_pk(&mut conn, partition_id, keys, after, before, first, last).await
    }

    async fn event(&self, ctx: &Context<'_>, id: ID) -> Result<Event> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
        event_by_id(&mut conn, id.0.to_string()).await
    }

    async fn events(
        &self,
        ctx: &Context<'_>,
        keys: Vec<String>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<OpaqueCursor<ID>, Event>> {
        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
        events_by_keys(&mut conn, &keys, after, before, first, last).await
    }
}
