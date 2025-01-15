use anyhow::{Context, Error, Result};
use async_trait::async_trait;
use dojo_types::schema::{Struct, Ty};
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use torii_sqlite::Sql;
use tracing::{debug, info};

use super::{EventProcessor, EventProcessorConfig};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::store_update_member";

#[derive(Default, Debug)]
pub struct StoreUpdateMemberProcessor;

#[async_trait]
impl<P> EventProcessor<P> for StoreUpdateMemberProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "StoreUpdateMember".to_string()
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
                <StoreUpdateMemberProcessor as EventProcessor<P>>::event_key(self)
            )
        }) {
            WorldEvent::StoreUpdateMember(e) => e,
            _ => {
                unreachable!()
            }
        };

        let model_selector = event.selector;
        let entity_id = event.entity_id;
        let member_selector = event.member_selector;

        // If the model does not exist, silently ignore it.
        // This can happen if only specific namespaces are indexed.
        let model = match db.model(model_selector).await {
            Ok(m) => m,
            Err(e) if e.to_string().contains("no rows") && !config.namespaces.is_empty() => {
                debug!(
                    target: LOG_TARGET,
                    selector = %model_selector,
                    "Model does not exist, skipping."
                );
                return Ok(());
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to retrieve model with selector {:#x}: {}",
                    event.selector,
                    e
                ))
            }
        };

        let schema = model.schema;

        let mut member = schema
            .as_struct()
            .expect("model schema must be a struct")
            .children
            .iter()
            .find(|c| {
                get_selector_from_name(&c.name).expect("invalid selector for member name")
                    == member_selector
            })
            .context("member not found")?
            .clone();

        info!(
            target: LOG_TARGET,
            name = %model.name,
            entity_id = format!("{:#x}", entity_id),
            member = %member.name,
            "Store update member.",
        );

        let mut values = event.values.to_vec();
        member.ty.deserialize(&mut values)?;

        let wrapped_ty = Ty::Struct(Struct { name: schema.name(), children: vec![member] });
        db.set_entity(wrapped_ty, event_id, block_timestamp, entity_id, model_selector, None)
            .await?;
        Ok(())
    }
}
