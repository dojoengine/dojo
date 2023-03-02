use juniper::{graphql_object, GraphQLObject};
use juniper_relay_connection::RelayConnectionNode;

use super::entity::Entity;
use super::entity_state::EntityState;
use super::entity_state_update::EntityStateUpdate;
use super::system::System;
use super::Query;
use crate::prisma::{component, PrismaClient};

#[derive(GraphQLObject)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub systems: Vec<System>,
    pub states: Vec<EntityState>,
    pub state_updates: Vec<EntityStateUpdate>,
    pub transaction_hash: String,
}

impl RelayConnectionNode for Component {
    type Cursor = String;
    fn cursor(&self) -> Self::Cursor {
        self.id.clone()
    }

    fn connection_type_name() -> &'static str {
        "Component"
    }

    fn edge_type_name() -> &'static str {
        "ComponentEdge"
    }
}

#[graphql_object(context = PrismaClient)]
impl Query {
    async fn component(context: &PrismaClient, id: String) -> Option<Component> {
        let component =
            context.component().find_first(vec![component::id::equals(id)]).exec().await.unwrap();

        match component {
            Some(component) => Some(Component {
                id: component.clone().id,
                name: component.clone().name,
                transaction_hash: component.clone().transaction_hash,
                systems: component
                    .clone()
                    .systems
                    .unwrap()
                    .into_iter()
                    .map(|system| System {
                        id: system.id,
                        name: system.name,
                        transaction_hash: system.transaction_hash,
                        query_components: vec![],
                        calls: vec![],
                    })
                    .collect(),
                states: component
                    .clone()
                    .states
                    .unwrap()
                    .into_iter()
                    .map(|state| EntityState {
                        data: state.data,
                        entity: Entity {
                            id: state.entity.clone().unwrap().id,
                            transaction_hash: state.entity.clone().unwrap().transaction_hash,
                            states: vec![],
                            state_updates: vec![],
                        },
                        component: Component {
                            id: component.clone().id,
                            name: component.clone().name,
                            transaction_hash: component.clone().transaction_hash,
                            systems: vec![],
                            states: vec![],
                            state_updates: vec![],
                        },
                    })
                    .collect(),
                state_updates: component
                    .clone()
                    .state_updates
                    .unwrap()
                    .into_iter()
                    .map(|state_update| EntityStateUpdate {
                        id: state_update.id,
                        data: state_update.data,
                        transaction_hash: state_update.transaction_hash,
                        entity: Entity {
                            id: state_update.entity.clone().unwrap().id,
                            transaction_hash: state_update.entity.clone().unwrap().transaction_hash,
                            states: vec![],
                            state_updates: vec![],
                        },
                        component: Component {
                            id: component.clone().id,
                            name: component.clone().name,
                            transaction_hash: component.clone().transaction_hash,
                            systems: vec![],
                            states: vec![],
                            state_updates: vec![],
                        },
                    })
                    .collect(),
            }),
            None => None,
        }
    }
}
