use juniper::{graphql_object, GraphQLObject};
use juniper_relay_connection::{RelayConnectionNode, RelayConnection};
use prisma_client_rust::QueryError;

use crate::prisma::{PrismaClient, component, system, entity};

use super::Query;

#[derive(GraphQLObject)]
pub struct Entity {
    pub id: String,
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
    async fn entity(
        context: &PrismaClient,
        id: String,
    ) -> Option<Entity> {
        let entity = context
            .entity()
            .find_first(vec![entity::id::equals(id)])
            .exec()
            .await
            .unwrap();

        match entity {
            Some(entity) => Some(Entity { id: entity.id, transaction_hash: entity.transaction_hash }),
            None => None,
        }
    }
}