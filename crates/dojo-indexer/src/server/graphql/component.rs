use juniper::{graphql_object, GraphQLObject};
use prisma_client_rust::QueryError;

use crate::prisma::{PrismaClient, component};

use super::Query;

#[derive(GraphQLObject)]
struct Component {
    id: String,
    name: String,
    transaction_hash: String,
}

#[graphql_object(context = PrismaClient)]
impl Query {
    async fn component(
        context: &PrismaClient,
        id: String,
    ) -> Option<Component> {
        let component = context
            .component()
            .find_first(vec![component::id::equals(id)])
            .exec()
            .await
            .unwrap();

        match component {
            Some(component) => Some(Component { id: component.id, name: component.name, transaction_hash: component.transaction_hash }),
            None => None,
        }
    }
}