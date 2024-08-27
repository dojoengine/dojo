use anyhow::{Context, Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use num_traits::ToPrimitive;
use starknet::core::types::{Event, TransactionReceiptWithBlockInfo};
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::{MODEL_INDEX, NUM_KEYS_INDEX};
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::store_set_record";

#[derive(Default, Debug)]
pub struct StoreSetRecordProcessor;

#[async_trait]
impl<P> EventProcessor<P> for StoreSetRecordProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "StoreSetRecord".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                target: LOG_TARGET,
                event_key = %<StoreSetRecordProcessor as EventProcessor<P>>::event_key(self),
                invalid_keys = %<StoreSetRecordProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
                "Invalid event keys."
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        block_timestamp: u64,
        _transaction_receipt: &TransactionReceiptWithBlockInfo,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let selector = event.keys[MODEL_INDEX];

        let model = db.model(selector).await?;

        info!(
            target: LOG_TARGET,
            name = %model.name,
            "Store set record.",
        );

        let keys_start = NUM_KEYS_INDEX + 1;
        let keys_end: usize =
            keys_start + event.keys[NUM_KEYS_INDEX].to_usize().context("invalid usize")?;
        let keys = event.keys[keys_start..keys_end].to_vec();

        // Skip the length to only get the values as they will be deserialized.
        let values = event.data[1..].to_vec();

        let mut keys_and_unpacked = [keys, values].concat();

        let mut entity = model.schema;
        entity.deserialize(&mut keys_and_unpacked)?;

        db.set_entity(entity, event_id, block_timestamp).await?;
        Ok(())
    }
}
