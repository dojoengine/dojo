use juniper::GraphQLObject;
use juniper_relay_connection::RelayConnectionNode;

use super::component::Component;
use super::entity_state::EntityState;
use super::entity_state_update::EntityStateUpdate;
use super::Query;
use crate::prisma::{entity, PrismaClient};

#[derive(GraphQLObject)]
pub struct Entity {
    pub id: String,
    pub states: Vec<EntityState>,
    pub state_updates: Vec<EntityStateUpdate>,
    pub transaction_hash: String,
}

impl RelayConnectionNode for Entity {
    type Cursor = String;
    fn cursor(&self) -> Self::Cursor {
        self.id.clone()
    }

    fn connection_type_name() -> &'static str {
        "Entity"
    }

    fn edge_type_name() -> &'static str {
        "EntityEdge"
    }
}

impl Query {
    #[allow(dead_code)]
    async fn entity(context: &PrismaClient, id: String) -> Option<Entity> {
        let entity =
            context.entity().find_first(vec![entity::id::equals(id)]).exec().await.unwrap();

        match entity {
            Some(entity) => Some(Entity {
                id: entity.id,
                transaction_hash: entity.transaction_hash,
                states: entity
                    .states
                    .unwrap()
                    .into_iter()
                    .map(|state| EntityState {
                        data: state.data,
                        entity: Entity {
                            id: state.entity.clone().unwrap().id,
                            transaction_hash: state.entity.unwrap().transaction_hash,
                            states: vec![],
                            state_updates: vec![],
                        },
                        component: Component {
                            id: state.component.clone().unwrap().id,
                            name: state.component.clone().unwrap().name,
                            transaction_hash: state.component.unwrap().transaction_hash,
                            systems: vec![],
                            states: vec![],
                            state_updates: vec![],
                        },
                    })
                    .collect(),
                state_updates: entity
                    .state_updates
                    .unwrap()
                    .into_iter()
                    .map(|state_update| EntityStateUpdate {
                        id: state_update.id,
                        data: state_update.data,
                        entity: Entity {
                            id: state_update.entity.clone().unwrap().id,
                            transaction_hash: state_update.entity.unwrap().transaction_hash,
                            states: vec![],
                            state_updates: vec![],
                        },
                        component: Component {
                            id: state_update.component.clone().unwrap().id,
                            name: state_update.component.clone().unwrap().name,
                            transaction_hash: state_update
                                .component
                                .unwrap()
                                .transaction_hash,
                            systems: vec![],
                            states: vec![],
                            state_updates: vec![],
                        },
                        transaction_hash: state_update.transaction_hash,
                    })
                    .collect(),
            }),
            None => None,
        }
    }
}
