use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, TransactionReceiptWithBlockInfo};
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::processors::{MODEL_INDEX, NUM_KEYS_INDEX};
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::store_del_record";

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
                target: LOG_TARGET,
                event_key = %<StoreDelRecordProcessor as EventProcessor<P>>::event_key(self),
                invalid_keys = %<StoreDelRecordProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
                "Invalid event keys."
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
        _block_timestamp: u64,
        _transaction_receipt: &TransactionReceiptWithBlockInfo,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let selector = event.data[MODEL_INDEX];

        let model = db.model(&format!("{:#x}", selector)).await?;

        info!(
            target: LOG_TARGET,
            name = %model.name(),
            "Store delete record."
        );

        let keys_start = NUM_KEYS_INDEX + 1;
        let keys = event.data[keys_start..].to_vec();
        let entity = model.schema().await?;
        db.delete_entity(keys, entity).await?;
        Ok(())
    }
}
