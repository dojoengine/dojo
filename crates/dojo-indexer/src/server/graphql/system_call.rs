use juniper::{GraphQLObject};
use juniper_relay_connection::{RelayConnectionNode};


use super::system::System;
use super::Query;
use crate::prisma::{system_call, PrismaClient};

#[derive(GraphQLObject)]
pub struct SystemCall {
    pub id: i32,
    pub system: System,
    pub data: String,
    pub transaction_hash: String,
}

impl RelayConnectionNode for SystemCall {
    type Cursor = i32;
    fn cursor(&self) -> Self::Cursor {
        self.id
    }

    fn connection_type_name() -> &'static str {
        "SystemCall"
    }

    fn edge_type_name() -> &'static str {
        "SystemCallEdge"
    }
}

impl Query {
    async fn system_call(context: &PrismaClient, id: i32) -> Option<SystemCall> {
        let call = context
            .system_call()
            .find_first(vec![system_call::id::equals(id)])
            .exec()
            .await
            .unwrap();

        match call {
            Some(call) => Some(SystemCall {
                id: call.id,
                data: call.data,
                transaction_hash: call.transaction_hash,
                system: System {
                    id: call.system.clone().unwrap().id,
                    name: call.system.clone().unwrap().name,
                    transaction_hash: call.system.clone().unwrap().transaction_hash,
                    calls: vec![],
                    query_components: vec![],
                },
            }),
            None => None,
        }
    }
}
