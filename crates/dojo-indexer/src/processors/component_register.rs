use std::{io::Read, cmp::Ordering};

use anyhow::{Result, Error, Ok};
use apibara_client_protos::pb::starknet::v1alpha2::EventWithTransaction;
use prisma_client_rust::bigdecimal::num_bigint::BigUint;
use sha3::{Keccak256, Digest};
use tonic::async_trait;


use crate::prisma;
use crate::hash::starknet_hash;

use super::{IProcessor};
pub struct ComponentRegistrationProcessor;
impl ComponentRegistrationProcessor {
    pub fn new() -> Self {
        Self {}
    }
}
#[async_trait]
impl IProcessor<EventWithTransaction> for ComponentRegistrationProcessor {
    async fn process(&self, client: prisma::PrismaClient, data: EventWithTransaction) -> Result<(), Error> {
        let event = &data.event.unwrap();
        let event_key = &event.keys[0].to_biguint();
        if (event_key.cmp(&starknet_hash(b"ComponentRegistered")) != Ordering::Equal) {
            return Ok(());
        }

        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let component = &event.data[0].to_biguint();

        // create a new component
        let component = client.component().create(
            "0x".to_owned()+component.to_str_radix(16).as_str(), 
            "Component".to_string(),
            "0x".to_owned()+transaction_hash.to_str_radix(16).as_str(),
            vec![]
        ).exec().await;

        Ok(())
    }
}