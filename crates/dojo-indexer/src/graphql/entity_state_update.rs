use juniper::GraphQLObject;

use super::component::Component;
use super::entity::Entity;

#[derive(GraphQLObject)]
pub struct EntityStateUpdate {
    pub id: i32,
    pub entity: Entity,
    pub component: Component,
    pub data: String,
    pub transaction_hash: String,
}
