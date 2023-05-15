pub mod component;
pub mod entity;
pub mod entity_state;
pub mod entity_state_update;
pub mod event;
pub mod server;
pub mod system;
pub mod system_call;

use component::Component;
use entity::Entity;
use event::Event;
use juniper::{graphql_object, FieldResult};
use server::Context;
use system::System;

pub struct Query;

#[graphql_object(context = Context)]
impl Query {
    async fn component(context: &Context, id: String) -> FieldResult<Component> {
        component::component(context, id).await
    }

    async fn components(context: &Context) -> FieldResult<Vec<Component>> {
        component::components(context).await
    }

    async fn system(context: &Context, id: String) -> FieldResult<System> {
        system::system(context, id).await
    }

    async fn systems(context: &Context) -> FieldResult<Vec<System>> {
        system::systems(context).await
    }

    // async fn entity_state_update(context: &Context, id: i64) ->
    // FieldResult<entity_state_update::EntityStateUpdate> {
    //     entity_state_update::entity_state_update(context, id).await
    // }

    // async fn entity_state_updates(context: &Context) ->
    // FieldResult<Vec<entity_state_update::EntityStateUpdate>> {
    //     entity_state_update::entity_state_updates(context).await
    // }

    async fn entity(context: &Context, id: String) -> FieldResult<Entity> {
        entity::entity(context, id).await
    }

    async fn entities(context: &Context) -> FieldResult<Vec<Entity>> {
        entity::entities(context).await
    }

    async fn entities_by_partition_id(
        context: &Context,
        partition_id: String,
    ) -> FieldResult<Vec<Entity>> {
        entity::entities_by_partition_id(context, partition_id).await
    }

    async fn entity_by_partition_id_keys(
        context: &Context,
        partition_id: String,
        keys: Vec<String>,
    ) -> FieldResult<entity::Entity> {
        entity::entity_by_partition_id_keys(context, partition_id, keys).await
    }

    async fn event(context: &Context, id: String) -> FieldResult<Event> {
        event::event(context, id).await
    }

    async fn events_by_keys(context: &Context, keys: Vec<String>) -> FieldResult<Vec<Event>> {
        event::events_by_keys(context, keys).await
    }
}
