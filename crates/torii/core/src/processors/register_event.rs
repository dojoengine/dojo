use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::{debug, info};

use super::EventProcessor;
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::register_event";

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

    async fn process(
        &self,
        world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        block_timestamp: u64,
        _event_id: &str,
        event: &Event,
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

        // Called model here by language, but it's an event. Torii rework will make clear
        // distinction.
        let model = world.model_reader(&namespace, &name).await?;
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
            "Registered model content."
        );

        db.register_model(
            &namespace,
            schema,
            layout,
            event.class_hash.into(),
            event.address.into(),
            packed_size,
            unpacked_size,
            block_timestamp,
        )
        .await?;

        Ok(())
    }
}
