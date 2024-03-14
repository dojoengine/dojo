use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockWithTxs, Event, TransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::{MODEL_INDEX, NUM_KEYS_INDEX};
use crate::sql::Sql;

#[derive(Default)]
pub struct EventMessageProcessor;

#[async_trait]
impl<P> EventProcessor<P> for EventMessageProcessor
where
    P: Provider + Send + Sync,
{
    fn event_key(&self) -> String {
        "".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block: &BlockWithTxs,
        _transaction_receipt: &TransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        // silently ignore if the model is not found
        let model = match db.model(&format!("{:#x}", event.keys[MODEL_INDEX])).await {
            Ok(model) => model,
            Err(_) => return Ok(()),
        };

        let mut keys_and_unpacked = [event.keys.clone(), event.data.clone()].concat();

        let mut entity = model.schema().await?;
        entity.deserialize(&mut keys_and_unpacked)?;

        println!("entity: {:?}", entity);

        db.set_entity(entity, event_id).await?;
        Ok(())
    }
}
