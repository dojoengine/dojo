pub mod component;
pub mod entity;
pub mod entity_state;
pub mod entity_state_update;
pub mod server;
pub mod system;
pub mod system_call;

use component::Component;
use juniper::{graphql_object, FieldResult};
use server::Context;

pub struct Query;

#[graphql_object(context = Context)]
impl Query {
    async fn component(context: &Context, id: String) -> FieldResult<Component> {
        component::component(context, id).await
    }

    async fn components(context: &Context) -> FieldResult<Vec<Component>> {
        component::components(context).await
    }

    async fn system(context: &Context, id: String) -> FieldResult<system::System> {
        system::system(context, id).await
    }

    async fn systems(context: &Context) -> FieldResult<Vec<system::System>> {
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

    async fn entity(context: &Context, id: String) -> FieldResult<entity::Entity> {
        entity::entity(context, id).await
    }

    async fn entities(context: &Context) -> FieldResult<Vec<entity::Entity>> {
        entity::entities(context).await
    }

    async fn entities_by_partition_id(
        context: &Context,
        partition_id: String,
    ) -> FieldResult<Vec<entity::Entity>> {
        entity::entities_by_partition_id(context, partition_id).await
    }

    async fn entity_by_partition_id_keys(
        context: &Context,
        partition_id: String,
        partition_keys: String,
    ) -> FieldResult<entity::Entity> {
        entity::entity_by_partition_id_keys(context, partition_id, partition_keys).await
    }
}
