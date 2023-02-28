use juniper::{graphql_object, GraphQLObject};
use juniper_relay_connection::{RelayConnectionNode, RelayConnection};
use prisma_client_rust::QueryError;

use crate::prisma::{PrismaClient, component, system, entity, entity_state, entity_state_update};

use super::{Query, entity::Entity, component::Component};

#[derive(GraphQLObject)]
pub struct EntityStateUpdate {
    pub id: i32,
    pub entity: Entity,
    pub component: Component,
    pub data: String,
    pub transaction_hash: String,
}