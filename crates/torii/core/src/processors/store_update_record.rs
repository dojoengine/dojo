use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_types::schema::Ty;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::{debug, info};

use super::{EventProcessor, EventProcessorConfig};
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
                <StoreUpdateRecordProcessor as EventProcessor<P>>::event_key(self)
            )
        }) {
            WorldEvent::StoreUpdateRecord(e) => e,
            _ => {
                unreachable!()
            }
        };

        let model_selector = event.selector;
        let entity_id = event.entity_id;

        // If the model does not exist, silently ignore it.
        // This can happen if only specific namespaces are indexed.
        let model = match db.model(event.selector).await {
            Ok(m) => m,
            Err(e) if e.to_string().contains("no rows") => {
                debug!(
                    target: LOG_TARGET,
                    selector = %event.selector,
                    "Model does not exist, skipping."
                );
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        info!(
            target: LOG_TARGET,
            namespace = %model.namespace,
            name = %model.name,
            entity_id = format!("{:#x}", entity_id),
            "Store update record.",
        );

        let mut entity = model.schema;
        match entity {
            Ty::Struct(ref mut struct_) => {
                // we do not need the keys. the entity Ty has the keys in its schema
                // so we should get rid of them to avoid trying to deserialize them
                struct_.children.retain(|field| !field.key);
            }
            _ => return Err(anyhow::anyhow!("Expected struct")),
        }

        let mut values = event.values.to_vec();
        entity.deserialize(&mut values)?;

        db.set_entity(entity, event_id, block_timestamp, entity_id, model_selector, None).await?;
        Ok(())
    }
}
