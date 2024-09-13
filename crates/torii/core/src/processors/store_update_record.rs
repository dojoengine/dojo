use anyhow::{Context, Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::naming;
use dojo_world::contracts::world::WorldContractReader;
use num_traits::ToPrimitive;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::{ENTITY_ID_INDEX, MODEL_INDEX};
use crate::sql::{felts_sql_string, Sql};

pub(crate) const LOG_TARGET: &str = "torii_core::processors::store_update_record";

#[derive(Default, Debug)]
pub struct StoreUpdateRecordProcessor;

#[async_trait]
impl<P> EventProcessor<P> for StoreUpdateRecordProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "StoreUpdateRecord".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                target: LOG_TARGET,
                event_key = %<StoreUpdateRecordProcessor as EventProcessor<P>>::event_key(self),
                invalid_keys = %<StoreUpdateRecordProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
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
        let entity_id = event.data[ENTITY_ID_INDEX];

        let model = db.model(model_id).await?;

        info!(
            target: LOG_TARGET,
            name = %model.name,
            entity_id = format!("{:#x}", entity_id),
            "Store update record.",
        );

        let values_start = ENTITY_ID_INDEX + 1;
        let values_end: usize =
            values_start + event.data[values_start].to_usize().context("invalid usize")?;

        // Skip the length to only get the values as they will be deserialized.
        let values = event.data[values_start + 1..=values_end].to_vec();

        let tag = naming::get_tag(&model.namespace, &model.name);

        // Keys are read from the db, since we don't have access to them when only
        // the entity id is passed.

        let keys = db.get_entity_keys(entity_id, &tag).await.with_context(|| {
            format!("Failed to get keys for entity: {:#x}, model: {}", entity_id, tag)
        })?;

        let keys_str = felts_sql_string(&keys);
        let mut keys_and_unpacked = [keys, values].concat();

        let mut entity = model.schema;
        entity.deserialize(&mut keys_and_unpacked).with_context(|| {
            format!("Failed to deserialize entity: {}, schema: {}", model.name, &entity)
        })?;

        db.set_entity(entity, event_id, block_timestamp, entity_id, model_id, &keys_str).await?;
        Ok(())
    }
}
