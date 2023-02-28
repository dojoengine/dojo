use juniper::{GraphQLObject};



use super::component::Component;
use super::entity::Entity;



#[derive(GraphQLObject)]
pub struct EntityState {
    pub entity: Entity,
    pub component: Component,
    pub data: String,
}
