use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::{Error, Result};
use async_trait::async_trait;
use cainome::cairo_serde::CairoSerde;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::naming::get_tag;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, Felt};
use starknet::providers::Provider;
use starknet_crypto::poseidon_hash_many;
use torii_sqlite::Sql;
use tracing::info;

use super::{EventProcessor, EventProcessorConfig};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::event_message";

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

    fn task_priority(&self) -> usize {
        1
    }

    fn task_identifier(&self, event: &Event) -> u64 {
        let mut hasher = DefaultHasher::new();
        let keys = Vec::<Felt>::cairo_deserialize(&event.data, 0).unwrap_or_else(|e| {
            panic!("Expected EventEmitted keys to be well formed: {:?}", e);
        });
        // selector
        event.keys[1].hash(&mut hasher);
        // entity id
        let entity_id = poseidon_hash_many(&keys);
        entity_id.hash(&mut hasher);
        hasher.finish()
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

        let mut keys_and_unpacked = [event.keys, event.values].concat();

        let mut entity = model.schema.clone();
        entity.deserialize(&mut keys_and_unpacked)?;

        let historical = config.is_historical(&get_tag(&model.namespace, &model.name));
        db.set_event_message(entity, event_id, block_timestamp, historical).await?;
        Ok(())
    }
}
