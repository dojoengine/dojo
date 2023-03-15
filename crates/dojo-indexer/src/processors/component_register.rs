use std::cmp::Ordering;

use anyhow::{Error, Ok, Result};
use apibara_core::starknet::v1alpha2::EventWithTransaction;
use num::BigUint;
use sqlx::{Executor, Pool, Sqlite};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::models::{BlockId, BlockTag};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use tonic::async_trait;

use super::{EventProcessor, IProcessor};
use crate::hash::starknet_hash;
use crate::stream::FieldElementExt;

pub struct ComponentRegistrationProcessor;
impl ComponentRegistrationProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventProcessor for ComponentRegistrationProcessor {
    fn get_event_key(&self) -> String {
        "ComponentRegistered".to_string()
    }
}

#[async_trait]
impl IProcessor<EventWithTransaction> for ComponentRegistrationProcessor {
    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<HttpTransport>,
        data: EventWithTransaction,
    ) -> Result<(), Error> {
        let event = &data.event.unwrap();
        let event_key = &event.keys[0].to_biguint();
        if event_key.cmp(&starknet_hash(self.get_event_key().as_bytes())) != Ordering::Equal {
            return Ok(());
        }

        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let component = &event.data[0].to_biguint();

        let class_hash = provider
            .get_class_hash_at(
                &BlockId::Tag(BlockTag::Latest),
                FieldElement::from_bytes_be(component.to_bytes_be().as_slice().try_into().unwrap())
                    .unwrap(),
            )
            .await;
        if class_hash.is_err() {
            return Err(Error::msg("Getting class hash."));
        }

        let address = "0x".to_owned() + component.to_str_radix(16).as_str();
        let txn_hash = "0x".to_owned() + transaction_hash.to_str_radix(16).as_str();
        let class_hash = "0x".to_owned()
            + BigUint::from_bytes_be(class_hash.unwrap().to_bytes_be().as_slice())
                .to_str_radix(16)
                .as_str();

        let mut tx = pool.begin().await?;
        tx.execute(sqlx::query!(
            "
            INSERT INTO components (id, name, properties, address, class_hash, transaction_hash)
            VALUES ($1, $2, $3, $4, $5, $6)
            ",
            "Component",
            address,
            "",
            address,
            class_hash,
            txn_hash,
        ))
        .await?;

        tx.commit().await?;

        Ok(())
    }
}
