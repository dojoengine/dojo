use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, Felt};
use starknet::providers::Provider;
use tracing::info;

use super::{EventProcessor, EventProcessorConfig};
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::event_message";

#[derive(Default, Debug)]
pub struct EventMessageProcessor;

#[async_trait]
impl<P> EventProcessor<P> for EventMessageProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "EventEmitted".to_string()
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
        config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        // Torii version is coupled to the world version, so we can expect the event to be well
        // formed.
        let event = match WorldEvent::try_from(event).unwrap_or_else(|_| {
            panic!(
                "Expected {} event to be well formed.",
                <EventMessageProcessor as EventProcessor<P>>::event_key(self)
            )
        }) {
            WorldEvent::EventEmitted(e) => e,
            _ => {
                unreachable!()
            }
        };

        // silently ignore if the model is not found
        let model = match db.model(event.selector).await {
            Ok(model) => model,
            Err(_) => return Ok(()),
        };

        info!(
            target: LOG_TARGET,
            namespace = %model.namespace,
            name = %model.name,
            system = %format!("{:#x}", Felt::from(event.system_address)),
            "Store event message."
        );

        // TODO: check historical and keep the internal counter.

        let mut keys_and_unpacked = [event.keys, event.values].concat();

        let mut entity = model.schema.clone();
        entity.deserialize(&mut keys_and_unpacked)?;

        // TODO: this must come from some torii's configuration.
        let historical =
            config.historical_events.contains(&format!("{}-{}", model.namespace, model.name));
        db.set_event_message(entity, event_id, block_timestamp, historical).await?;
        Ok(())
    }
}
