use std::cmp::Ordering;

use anyhow::{Error, Ok, Result};
use apibara_client_protos::pb::starknet::v1alpha2::EventWithTransaction;

use tonic::async_trait;

use crate::hash::starknet_hash;
use crate::prisma;

use super::{EventProcessor, IProcessor};
pub struct ComponentRegistrationProcessor;
impl ComponentRegistrationProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventProcessor for ComponentRegistrationProcessor {
    fn get_event_key(&self) -> String {
        "SystemRegistered".to_string()
    }
}

#[async_trait]
impl IProcessor<EventWithTransaction> for ComponentRegistrationProcessor {
    async fn process(
        &self,
        client: &prisma::PrismaClient,
        data: EventWithTransaction,
    ) -> Result<(), Error> {
        let event = &data.event.unwrap();
        let event_key = &event.keys[0].to_biguint();
        if event_key.cmp(&starknet_hash(self.get_event_key().as_bytes())) != Ordering::Equal {
            return Ok(());
        }

        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let system = &event.data[0].to_biguint();

        // create a new component
        let _system = client
            .system()
            .create(
                "0x".to_owned() + system.to_str_radix(16).as_str(),
                "System".to_string(),
                "0x".to_owned() + transaction_hash.to_str_radix(16).as_str(),
                vec![],
            )
            .exec()
            .await;

        Ok(())
    }
}
