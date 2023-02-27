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

impl Query {
    async fn entity_state_update(
        context: &PrismaClient,
        entity_id: String,
        component_id: String,
    ) -> Option<EntityStateUpdate> {
        let state = context
            .entity_state_update()
            .find_first(vec![entity_state_update::entity_id::equals(entity_id), entity_state_update::component_id::equals(component_id)])
            .exec()
            .await
            .unwrap();

        match state {
            Some(state) => Some(EntityStateUpdate { id: state.id, transaction_hash: state.transaction_hash, data: state.data, entity: Entity {
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