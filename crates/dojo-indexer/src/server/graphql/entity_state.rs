use juniper::{graphql_object, GraphQLObject};
use juniper_relay_connection::{RelayConnection, RelayConnectionNode};
use prisma_client_rust::QueryError;

use super::component::Component;
use super::entity::Entity;
use super::Query;
use crate::prisma::{component, entity, entity_state, system, PrismaClient};

#[derive(GraphQLObject)]
pub struct EntityState {
    pub entity: Entity,
    pub component: Component,
    pub data: String,
}
