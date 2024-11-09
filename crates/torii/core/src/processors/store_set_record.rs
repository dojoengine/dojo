use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::info;

use super::{EventProcessor, EventProcessorConfig};
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

    fn validate(&self, _event: &Event) -> bool {
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
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        // Torii version is coupled to the world version, so we can expect the event to be well
        // formed.
        let event = match WorldEvent::try_from(event).unwrap_or_else(|_| {
            panic!(
                "Expected {} event to be well formed.",
                <StoreSetRecordProcessor as EventProcessor<P>>::event_key(self)
            )
        }) {
            WorldEvent::StoreSetRecord(e) => e,
            _ => {
                unreachable!()
            }
        };

        let model = db.model(event.selector).await?;

        info!(
            target: LOG_TARGET,
            namespace = %model.namespace,
            name = %model.name,
            entity_id = format!("{:#x}", event.entity_id),
            "Store set record.",
        );

        let keys_str = felts_to_sql_string(&event.keys);

        let mut keys_and_unpacked = [event.keys, event.values].concat();

        let mut entity = model.schema;
        entity.deserialize(&mut keys_and_unpacked)?;

        db.set_entity(
            entity,
            event_id,
            block_timestamp,
            event.entity_id,
            event.selector,
            Some(&keys_str),
        )
        .await?;
        Ok(())
    }
}
