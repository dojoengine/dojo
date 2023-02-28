use std::vec;

use juniper::{graphql_object, GraphQLObject};
use juniper_relay_connection::{RelayConnection, RelayConnectionNode};
use prisma_client_rust::QueryError;

use super::component::Component;
use super::entity::Entity;
use super::entity_state::EntityState;
use super::entity_state_update::EntityStateUpdate;
use super::system_call::SystemCall;
use super::Query;
use crate::prisma::{component, system, PrismaClient};

#[derive(GraphQLObject)]
pub struct System {
    pub id: String,
    pub name: String,
    pub calls: Vec<SystemCall>,
    pub query_components: Vec<Component>,
    pub transaction_hash: String,
}

impl RelayConnectionNode for System {
    type Cursor = String;
    fn cursor(&self) -> Self::Cursor {
        self.id.clone()
    }

    fn connection_type_name() -> &'static str {
        "System"
    }

    fn edge_type_name() -> &'static str {
        "SystemEdge"
    }
}

impl Query {
    async fn system(context: &PrismaClient, id: String) -> Option<System> {
        let system =
            context.system().find_first(vec![system::id::equals(id)]).exec().await.unwrap();

        match system {
            Some(system) => Some(System {
                id: system.id.clone(),
                name: system.name.clone(),
                transaction_hash: system.transaction_hash.clone(),

                calls: system
                    .calls
                    .unwrap()
                    .into_iter()
                    .map(|call| SystemCall {
                        id: call.id,
                        data: call.data,
                        transaction_hash: call.transaction_hash,
                        system: System {
                            id: system.id.clone(),
                            name: system.name.clone(),
                            transaction_hash: system.transaction_hash.clone(),
                            calls: vec![],
                            query_components: vec![],
                        },
                    })
                    .collect(),
                query_components: system
                    .query_components
                    .unwrap()
                    .into_iter()
                    .map(|component| Component {
                        id: component.clone().id,
                        name: component.clone().name,
                        transaction_hash: component.clone().transaction_hash,
                        systems: vec![],
                        states: component
                            .states
                            .unwrap()
                            .into_iter()
                            .map(|state| EntityState {
                                data: state.data,
                                component: Component {
                                    id: component.id.clone(),
                                    name: component.name.clone(),
                                    transaction_hash: component.transaction_hash.clone(),
                                    systems: vec![],
                                    states: vec![],
                                    state_updates: vec![],
                                },
                                entity: Entity {
                                    id: state.entity.clone().unwrap().id,
                                    transaction_hash: state
                                        .entity
                                        .clone()
                                        .unwrap()
                                        .transaction_hash,
                                    state_updates: vec![],
                                    states: vec![],
                                },
                            })
                            .collect(),
                        state_updates: component
                            .state_updates
                            .unwrap()
                            .into_iter()
                            .map(|state_update| EntityStateUpdate {
                                id: state_update.id,
                                data: state_update.data,
                                transaction_hash: state_update.transaction_hash,
                                component: Component {
                                    id: component.id.clone(),
                                    name: component.name.clone(),
                                    transaction_hash: component.transaction_hash.clone(),
                                    systems: vec![],
                                    states: vec![],
                                    state_updates: vec![],
                                },
                                entity: Entity {
                                    id: state_update.entity.clone().unwrap().id,
                                    states: vec![],
                                    state_updates: vec![],
                                    transaction_hash: state_update
                                        .entity
                                        .clone()
                                        .unwrap()
                                        .transaction_hash,
                                },
                            })
                            .collect(),
                    })
                    .collect(),
            }),
            None => None,
        }
    }
}
