use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, TransactionReceipt};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::{MODEL_INDEX, NUM_KEYS_INDEX};
use crate::sql::Sql;

#[derive(Default)]
pub struct StoreDelRecordProcessor;

#[async_trait]
impl<P> EventProcessor<P> for StoreDelRecordProcessor
where
    P: Provider + Send + Sync,
{
    fn event_key(&self) -> String {
        "StoreDelRecord".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                "invalid keys for event {}: {}",
                <StoreDelRecordProcessor as EventProcessor<P>>::event_key(self),
                <StoreDelRecordProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        _transaction_receipt: &TransactionReceipt,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[MODEL_INDEX])?;
        info!("store delete record: {}", name);

        // this is temporary until the model name hash is precomputed
        let model = db.model(&format!("{:#x}", get_selector_from_name(&name)?)).await?;

        let keys_start = NUM_KEYS_INDEX + 1;
        let keys = event.data[keys_start..].to_vec();
        let entity = model.schema().await?;
        db.delete_entity(keys, entity).await?;
        Ok(())
    }
}
