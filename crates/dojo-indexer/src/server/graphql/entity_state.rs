use juniper::{graphql_object, GraphQLObject};
use juniper_relay_connection::{RelayConnectionNode, RelayConnection};
use prisma_client_rust::QueryError;

use crate::prisma::{PrismaClient, component, system, entity, entity_state};

use super::{Query, entity::Entity, component::Component};

#[derive(GraphQLObject)]
pub struct EntityState {
    pub entity: Entity,
    pub component: Component,
    pub data: String,
}