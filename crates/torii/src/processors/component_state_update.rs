use anyhow::{Error, Ok, Result};
use apibara_core::starknet::v1alpha2::EventWithTransaction;
use sqlx::{Executor, Pool, Sqlite};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tonic::async_trait;

use super::EventProcessor;
use crate::stream::FieldElementExt;

#[derive(Default)]
pub struct ComponentStateUpdateProcessor;

#[async_trait]
impl<T: JsonRpcTransport + Sync + Send> EventProcessor<T> for ComponentStateUpdateProcessor {
    fn event_key(&self) -> String {
        "ComponentStateUpdate".to_string()
    }

    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        _provider: &JsonRpcClient<T>,
        data: EventWithTransaction,
    ) -> Result<(), Error> {
        let event = &data.event.unwrap();
        let transaction_hash = &data.transaction.unwrap().meta.unwrap().hash.unwrap().to_biguint();
        let entity = &event.data[0].to_biguint();
        let component = &event.data[1].to_biguint();
        let data = &event.data[2].to_biguint();

        let entity_id = entity.to_string();
        let component_address = "0x".to_owned() + component.to_str_radix(16).as_str();
        let txn_hash = "0x".to_owned() + transaction_hash.to_str_radix(16).as_str();
        let parsed_data = data.to_string();

        let mut tx = pool.begin().await?;

        // create entity if doesn't exist
        tx.execute(sqlx::query!(
            "
            INSERT INTO entities (id, transaction_hash)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            ",
            entity_id,
            txn_hash,
        ));

        // insert entity state update
        tx.execute(sqlx::query!(
            "
            INSERT INTO entity_state_updates (entity_id, component_id, transaction_hash, data)
            VALUES ($1, $2, $3, $4)
            ",
            entity_id,
            component_address,
            txn_hash,
            parsed_data,
        ))
        .await?;

        // insert or update entity state
        tx.execute(sqlx::query!(
            "
            INSERT INTO entity_states (entity_id, component_id, data)
            VALUES ($1, $2, $3)
            ON CONFLICT (entity_id, component_id) DO UPDATE SET data = $3
            ",
            entity_id,
            component_address,
            parsed_data,
        ))
        .await?;

        tx.commit().await?;

        Ok(())
    }
}
