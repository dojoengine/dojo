use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::MODEL_INDEX;
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
        "".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // we expect at least 3 keys
        // 1: event selector
        // 2: model keys, arbitrary length
        // last key: system key
        if event.keys.len() < 3 {
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
        // silently ignore if the model is not found
        let model = match db.model(event.keys[MODEL_INDEX]).await {
            Ok(model) => model,
            Err(_) => return Ok(()),
        };

        info!(
            target: LOG_TARGET,
            model = %model.name,
            "Store event message."
        );

        // skip the first key, as its the event selector
        // and dont include last key as its the system key
        let mut keys_and_unpacked =
            [event.keys[1..event.keys.len() - 1].to_vec(), event.data.clone()].concat();

        let mut entity = model.schema.clone();
        entity.deserialize(&mut keys_and_unpacked)?;

        db.set_event_message(entity, event_id, block_timestamp).await?;
        Ok(())
    }
}
