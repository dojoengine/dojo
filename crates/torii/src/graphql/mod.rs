pub mod component;
pub mod entity;
pub mod entity_state;
pub mod event;
pub mod server;
pub mod system;
pub mod system_call;

use async_graphql::{Context, Object, Result};
use component::Component;
use entity::Entity;
use event::Event;
use system::System;

pub struct Query;

#[Object]
impl Query {
    async fn component(&self, ctx: &Context<'_>, id: String) -> Result<Component> {
        component::component(ctx, id).await
    }

    async fn components(&self, ctx: &Context<'_>) -> Result<Vec<Component>> {
        component::components(ctx).await
    }

    async fn system(&self, ctx: &Context<'_>, id: String) -> Result<System> {
        system::system(ctx, id).await
    }

    async fn systems(&self, ctx: &Context<'_>) -> Result<Vec<System>> {
        system::systems(ctx).await
    }

    async fn entity(&self, ctx: &Context<'_>, id: String) -> Result<Entity> {
        entity::entity(ctx, id).await
    }

    async fn entities(
        &self,
        ctx: &Context<'_>,
        partition_id: String,
        keys: Option<Vec<String>>,
    ) -> Result<Vec<Entity>> {
        entity::entities(ctx, partition_id, keys).await
    }

    async fn event(&self, ctx: &Context<'_>, id: String) -> Result<Event> {
        event::event(ctx, id).await
    }

    async fn events(&self, ctx: &Context<'_>, keys: Vec<String>) -> Result<Vec<Event>> {
        event::events(ctx, &keys).await
    }
}
