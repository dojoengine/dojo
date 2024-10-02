use anyhow::{Context, Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use num_traits::ToPrimitive;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::{ENTITY_ID_INDEX, MODEL_INDEX, NUM_KEYS_INDEX};
use crate::sql::utils::felts_to_sql_string;
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
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let model_id = event.data[MODEL_INDEX];

        let model = db.model(model_id).await?;

        info!(
            target: LOG_TARGET,
            name = %model.name,
            "Store set record.",
        );

        let keys_start = NUM_KEYS_INDEX + 1;
        let keys_end: usize =
            keys_start + event.data[NUM_KEYS_INDEX].to_usize().context("invalid usize")?;
        let keys = event.data[keys_start..keys_end].to_vec();
        let keys_str = felts_to_sql_string(&keys);

        // keys_end is already the length of the values array.

        let values_start = keys_end + 1;
        let values_end: usize =
            values_start + event.data[keys_end].to_usize().context("invalid usize")?;

        let values = event.data[values_start..values_end].to_vec();
        let entity_id = event.data[ENTITY_ID_INDEX];

        let mut keys_and_unpacked = [keys, values].concat();

        let mut entity = model.schema;
        entity.deserialize(&mut keys_and_unpacked)?;

        db.set_entity(entity, event_id, block_timestamp, entity_id, model_id, Some(&keys_str))
            .await?;
        Ok(())
    }
}
