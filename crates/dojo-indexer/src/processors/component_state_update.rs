use std::cmp::Ordering;

use anyhow::{Error, Ok, Result};
use apibara_client_protos::pb::starknet::v1alpha2::EventWithTransaction;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use tonic::async_trait;

use super::{EventProcessor, IProcessor};
use crate::hash::starknet_hash;
use crate::prisma;
pub struct ComponentStateUpdateProcessor;
impl ComponentStateUpdateProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventProcessor for ComponentStateUpdateProcessor {
    fn get_event_key(&self) -> String {
        "ComponentStateUpdate".to_string()
    }
}

#[async_trait]
impl IProcessor<EventWithTransaction> for ComponentStateUpdateProcessor {
    async fn process(
        &self,
        client: &prisma::PrismaClient,
        _provider: &JsonRpcClient<HttpTransport>,
        data: EventWithTransaction,
    ) -> Result<(), Error> {
        let event = &data.event.unwrap();
        let event_key = &event.keys[0].to_biguint();
        if event_key.cmp(&starknet_hash(self.get_event_key().as_bytes())) != Ordering::Equal {
            return Ok(());
        }

        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let entity = &event.data[0].to_biguint();
        let component = &event.data[1].to_biguint();
        let data = &event.data[2].to_biguint();

        // register a new state update
        let _state_update = client
            .entity_state_update()
            .create(
                prisma::entity::id::equals(entity.to_string()),
                "0x".to_owned() + transaction_hash.to_str_radix(16).as_str(),
                prisma::component::id::equals(
                    "0x".to_owned() + component.to_str_radix(16).as_str(),
                ),
                data.to_string(),
                vec![],
            )
            .exec()
            .await;

        let _state = client
            .entity_state()
            .create(
                prisma::entity::id::equals(entity.to_string()),
                prisma::component::id::equals(
                    "0x".to_owned() + component.to_str_radix(16).as_str(),
                ),
                data.to_string(),
                vec![],
            )
            .exec()
            .await;

        Ok(())
    }
}
