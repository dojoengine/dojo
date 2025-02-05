use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockId, Event};
use starknet::providers::Provider;
use torii_sqlite::Sql;
use tracing::{debug, info};

use super::{EventProcessor, EventProcessorConfig};
use crate::task_manager::{TaskId, TaskPriority};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::register_event";

#[derive(Default, Debug)]
pub struct RegisterEventProcessor;

#[async_trait]
impl<P> EventProcessor<P> for RegisterEventProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "EventRegistered".to_string()
    }

    // We might not need this anymore, since we don't have fallback and all world events must
    // be handled.
    fn validate(&self, _event: &Event) -> bool {
        true
    }

    fn task_priority(&self) -> TaskPriority {
        0
    }

    fn task_identifier(&self, event: &Event) -> TaskId {
        let mut hasher = DefaultHasher::new();
        event.keys.iter().for_each(|k| k.hash(&mut hasher));
        hasher.finish()
    }

    async fn process(
        &self,
        world: &WorldContractReader<P>,
        db: &mut Sql,
        block_number: u64,
        block_timestamp: u64,
        _event_id: &str,
        event: &Event,
        config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        // Torii version is coupled to the world version, so we can expect the event to be well
        // formed.
        let event = match WorldEvent::try_from(event).unwrap_or_else(|_| {
            panic!(
                "Expected {} event to be well formed.",
                <RegisterEventProcessor as EventProcessor<P>>::event_key(self)
            )
        }) {
            WorldEvent::EventRegistered(e) => e,
            _ => {
                unreachable!()
            }
        };

        // Safe to unwrap, since it's coming from the chain.
        let namespace = event.namespace.to_string().unwrap();
        let name = event.name.to_string().unwrap();

        // If the namespace is not in the list of namespaces to index, silently ignore it.
        // If our config is empty, we index all namespaces.
        if !config.should_index(&namespace) {
            return Ok(());
        }

        // Called model here by language, but it's an event. Torii rework will make clear
        // distinction.
        let model = if config.strict_model_reader {
            world.model_reader_with_block(&namespace, &name, BlockId::Number(block_number)).await?
        } else {
            world.model_reader(&namespace, &name).await?
        };
        let schema = model.schema().await?;
        let layout = model.layout().await?;

        // Events are never stored onchain, hence no packing or unpacking.
        let unpacked_size: u32 = 0;
        let packed_size: u32 = 0;

        info!(
            target: LOG_TARGET,
            namespace = %namespace,
            name = %name,
            "Registered event."
        );

        debug!(
            target: LOG_TARGET,
            name,
            schema = ?schema,
            layout = ?layout,
            class_hash = ?event.class_hash,
            contract_address = ?event.address,
            packed_size = %packed_size,
            unpacked_size = %unpacked_size,
            "Registered event content."
        );

        db.register_model(
            &namespace,
            &schema,
            layout,
            event.class_hash.into(),
            event.address.into(),
            packed_size,
            unpacked_size,
            block_timestamp,
            None,
        )
        .await?;

        Ok(())
    }
}
