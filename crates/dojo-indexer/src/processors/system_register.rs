use anyhow::{Error, Ok, Result};
use num::BigUint;
use sqlx::{Executor, Pool, Database};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::models::{BlockId, BlockTag, Event};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tonic::async_trait;

use super::EventProcessor;

#[derive(Default)]
pub struct SystemRegistrationProcessor<'a, DB: Database, T: JsonRpcTransport + Sync + Send> {
    pool: &'a Pool<DB>,
    provider: &'a JsonRpcClient<T>,
}

#[async_trait]
impl EventProcessor for SystemRegistrationProcessor {
    fn event_key(&self) -> String {
        "SystemRegistered".to_string()
    }

    async fn process(
        &self,
        event: Event,
    ) -> Result<(), Error> {
        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let system = &event.data[0].to_biguint();

        let class_hash: std::result::Result<FieldElement, starknet::providers::jsonrpc::JsonRpcClientError<_>> = self.provider
            .get_class_hash_at(
                &BlockId::Tag(BlockTag::Pending),
                FieldElement::from_bytes_be(system.to_bytes_be().as_slice().try_into().unwrap())
                    .unwrap(),
            )
            .await;
        if class_hash.is_err() {
            return Err(Error::msg("Getting class hash."));
        }

        let address = "0x".to_owned() + system.to_str_radix(16).as_str();
        let txn_hash = "0x".to_owned() + transaction_hash.to_str_radix(16).as_str();
        let class_hash = "0x".to_owned()
            + BigUint::from_bytes_be(class_hash.unwrap().to_bytes_be().as_slice())
                .to_str_radix(16)
                .as_str();

        // create a new system
        let mut tx = self.storage.begin().await?;
        tx.execute(sqlx::query!(
            "
            INSERT INTO systems (id, address, class_hash, transaction_hash)
            VALUES ($1, $2, $3, $4)
            ",
            address,
            address,
            class_hash,
            txn_hash,
        ))
        .await?;

        tx.commit().await?;

        Ok(())
    }
}
