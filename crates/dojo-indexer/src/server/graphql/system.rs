use juniper::{graphql_object, GraphQLObject};
use juniper_relay_connection::{RelayConnectionNode, RelayConnection};
use prisma_client_rust::QueryError;

use crate::prisma::{PrismaClient, component, system};

use super::Query;

#[derive(GraphQLObject)]
pub struct System {
    pub id: String,
    pub name: String,
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
    async fn system(
        context: &PrismaClient,
        id: String,
    ) -> Option<System> {
        let system = context
            .system()
            .find_first(vec![system::id::equals(id)])
            .exec()
            .await
            .unwrap();

        match system {
            Some(system) => Some(System { id: system.id, name: system.name, transaction_hash: system.transaction_hash }),
            None => None,
        }
    }
}