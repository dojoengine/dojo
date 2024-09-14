use anyhow::{Context, Error, Ok, Result};
use async_trait::async_trait;
use dojo_types::schema::Ty;
use dojo_world::contracts::world::WorldContractReader;
use num_traits::ToPrimitive;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::{ENTITY_ID_INDEX, MODEL_INDEX};
use crate::sql::Sql;

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
        let mut values = event.data[values_start + 1..=values_end].to_vec();

        let mut entity = model.schema;
        match entity {
            Ty::Struct(ref mut struct_) => {
                // we do not need the keys. the entity Ty has the keys in its schema
                // so we should get rid of them to avoid trying to deserialize them
                struct_.children.retain(|field| !field.key);
            }
            _ => return Err(anyhow::anyhow!("Expected struct")),
        }

        entity.deserialize(&mut values)?;

        db.set_entity(entity, event_id, block_timestamp, entity_id, model_id, None).await?;
        Ok(())
    }
}
