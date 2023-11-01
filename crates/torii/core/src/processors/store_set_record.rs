use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct StoreSetRecordProcessor;

const MODEL_INDEX: usize = 0;
const NUM_KEYS_INDEX: usize = 1;

#[async_trait]
impl<P> EventProcessor<P> for StoreSetRecordProcessor
where
    P::Error: 'static,
    P: Provider + Send + Sync,
{
    fn event_key(&self) -> String {
        "StoreSetRecord".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                "invalid keys for event {}: {}",
                <StoreSetRecordProcessor as EventProcessor<P>>::event_key(self),
                <StoreSetRecordProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block: &BlockWithTxs,
        _transaction_receipt: &InvokeTransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[MODEL_INDEX])?;
        info!("store set record: {}", name);

        let model = db.model(&name).await?;

        let keys_start = NUM_KEYS_INDEX + 1;
        let keys_end: usize = keys_start + usize::from(u8::try_from(event.data[NUM_KEYS_INDEX])?);
        let keys = event.data[keys_start..keys_end].to_vec();

        let values_start = keys_end + 2;
        let values_end: usize = values_start + usize::from(u8::try_from(event.data[keys_end + 1])?);

        let values = event.data[values_start..values_end].to_vec();
        let mut keys_and_unpacked = [keys, values].concat();

        let mut entity = model.schema().await?;
        entity.deserialize(&mut keys_and_unpacked)?;

        db.set_entity(entity, event_id).await?;
        Ok(())
    }
}
