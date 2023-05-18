use anyhow::{Error, Ok, Result};
use apibara_core::starknet::v1alpha2::EventWithTransaction;
use num::BigUint;
use sqlx::{Executor, Pool, Sqlite};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::models::{BlockId, BlockTag};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tonic::async_trait;

use super::EventProcessor;
use crate::stream::FieldElementExt;

#[derive(Default)]
pub struct ComponentRegistrationProcessor;

#[async_trait]
impl<T: JsonRpcTransport + Sync + Send> EventProcessor<T> for ComponentRegistrationProcessor {
    fn event_key(&self) -> String {
        "ComponentRegistered".to_string()
    }

    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<T>,
        data: EventWithTransaction,
    ) -> Result<(), Error> {
        let event = &data.event.unwrap();
        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let component = &event.data[0].to_biguint();

        let class_hash = provider
            .get_class_hash_at(
                &BlockId::Tag(BlockTag::Pending),
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
            INSERT INTO components (id, properties, address, class_hash, transaction_hash)
            VALUES ($1, $2, $3, $4, $5)
            ",
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
