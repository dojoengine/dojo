use std::cmp::Ordering;

use anyhow::{Error, Ok, Result};
use apibara_client_protos::pb::starknet::v1alpha2::EventWithTransaction;
use diesel::r2d2::{Pool, ConnectionManager};
use prisma_client_rust::bigdecimal::num_bigint::BigUint;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::models::{BlockId, BlockTag};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use tonic::async_trait;

use super::{EventProcessor, IProcessor};
use crate::hash::starknet_hash;
use crate::schema::DBConnection;
pub struct SystemRegistrationProcessor;
impl SystemRegistrationProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventProcessor for SystemRegistrationProcessor {
    fn get_event_key(&self) -> String {
        "SystemRegistered".to_string()
    }
}

#[async_trait]
impl IProcessor<EventWithTransaction> for SystemRegistrationProcessor {
    async fn process(
        &self,
        client: &Pool<ConnectionManager<DBConnection>>,
        provider: &JsonRpcClient<HttpTransport>,
        data: EventWithTransaction,
    ) -> Result<(), Error> {
        let event = &data.event.unwrap();
        let event_key = &event.keys[0].to_biguint();
        if event_key.cmp(&starknet_hash(self.get_event_key().as_bytes())) != Ordering::Equal {
            return Ok(());
        }

        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let system = &event.data[0].to_biguint();

        let class_hash = provider
            .get_class_hash_at(
                &BlockId::Tag(BlockTag::Latest),
                FieldElement::from_bytes_be(system.to_bytes_be().as_slice().try_into().unwrap())
                    .unwrap(),
            )
            .await;
        if class_hash.is_err() {
            return Err(Error::msg("Getting class hash."));
        }

        // create a new component
        // let _system = client
        //     .system()
        //     .create(
        //         "0x".to_owned() + system.to_str_radix(16).as_str(),
        //         "System".to_string(),
        //         "0x".to_owned() + system.to_str_radix(16).as_str(),
        //         "0x".to_owned()
        //             + BigUint::from_bytes_be(class_hash.unwrap().to_bytes_be().as_slice())
        //                 .to_str_radix(16)
        //                 .as_str(),
        //         "0x".to_owned() + transaction_hash.to_str_radix(16).as_str(),
        //         vec![],
        //     )
        //     .exec()
        //     .await;

        Ok(())
    }
}
