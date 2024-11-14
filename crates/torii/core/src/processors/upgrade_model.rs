use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::{debug, info};

use super::{EventProcessor, EventProcessorConfig};
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::upgrade_model";

#[derive(Default, Debug)]
pub struct UpgradeModelProcessor;

#[async_trait]
impl<P> EventProcessor<P> for UpgradeModelProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "ModelUpgraded".to_string()
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
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        // Torii version is coupled to the world version, so we can expect the event to be well
        // formed.
        let event = match WorldEvent::try_from(event).unwrap_or_else(|_| {
            panic!(
                "Expected {} event to be well formed.",
                <UpgradeModelProcessor as EventProcessor<P>>::event_key(self)
            )
        }) {
            WorldEvent::ModelUpgraded(e) => e,
            _ => {
                unreachable!()
            }
        };

        // If the model does not exist, silently ignore it.
        // This can happen if only specific namespaces are indexed.
        let model = match db.model(event.selector).await {
            Ok(m) => m,
            Err(e) => {
                if e.to_string().contains("no rows") {
                    return Ok(());
                }
                return Err(e);
            }
        };

        let name = model.name;
        let namespace = model.namespace;
        let prev_schema = model.schema;

        let model = world.model_reader(&namespace, &name).await?;
        let new_schema = model.schema().await?;
        let schema_diff = new_schema.diff(&prev_schema);
        // No changes to the schema. This can happen if torii is re-run with a fresh database.
        // As the register model fetches the latest schema from the chain.
        if schema_diff.is_none() {
            return Ok(());
        }

        let schema_diff = schema_diff.unwrap();
        let layout = model.layout().await?;

        let unpacked_size: u32 = model.unpacked_size().await?;
        let packed_size: u32 = model.packed_size().await?;

        info!(
            target: LOG_TARGET,
            namespace = %namespace,
            name = %name,
            "Upgraded model."
        );

        debug!(
            target: LOG_TARGET,
            name = %name,
            diff = ?schema_diff,
            layout = ?layout,
            class_hash = ?event.class_hash,
            contract_address = ?event.address,
            packed_size = %packed_size,
            unpacked_size = %unpacked_size,
            "Upgraded model content."
        );

        db.register_model(
            &namespace,
            &new_schema,
            layout,
            event.class_hash.into(),
            event.address.into(),
            packed_size,
            unpacked_size,
            block_timestamp,
            Some(&schema_diff),
        )
        .await?;

        Ok(())
    }
}
