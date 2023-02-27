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

impl Query {
    async fn entity_state(
        context: &PrismaClient,
        entity_id: String,
        component_id: String,
    ) -> Option<EntityState> {
        let state = context
            .entity_state()
            .find_first(vec![entity_state::entity_id::equals(entity_id), entity_state::component_id::equals(component_id)])
            .exec()
            .await
            .unwrap();

        match state {
            Some(state) => Some(EntityState { data: state.data, entity: Entity {
                id: state.entity.clone().unwrap().id,
                transaction_hash: state.entity.clone().unwrap().transaction_hash,
            }, component: Component {
                id: state.component.clone().unwrap().id,
                name: state.component.clone().unwrap().name,
                transaction_hash: state.component.clone().unwrap().transaction_hash,
            } }),
            None => None,
        }
    }
}