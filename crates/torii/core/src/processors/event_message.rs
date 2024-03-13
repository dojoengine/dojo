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
        let model = match parse_cairo_short_string(&event.keys[MODEL_INDEX]) {
            Ok(name) => {
                println!("name: {}", name);
                
                match db.model(&name).await {
                    Ok(model) => model,
                    Err(_) => return Ok(()),
                }
            }
            Err(_) => return Ok(()),
        };

        // let keys_start = NUM_KEYS_INDEX;
        // let keys_end: usize = keys_start + usize::from(u8::try_from(event.data[NUM_KEYS_INDEX])?);
        // let keys = event.keys[keys_start..keys_end].to_vec();

        // // keys_end is already the length of the values array.

        // let values_start = keys_end + 1;
        // let values_end: usize = values_start + usize::from(u8::try_from(event.data[keys_end])?);

        // let values = event.data[values_start..values_end].to_vec();
        let mut keys_and_unpacked = [event.keys.clone(), event.data.clone()].concat();

        let mut entity = model.schema().await?;
        entity.deserialize(&mut keys_and_unpacked)?;

        db.set_entity(entity, event_id).await?;
        Ok(())
    }
}
