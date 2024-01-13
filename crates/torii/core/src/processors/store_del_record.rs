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
pub struct StoreDelRecordProcessor;

const MODEL_INDEX: usize = 0;
const NUM_KEYS_INDEX: usize = 1;

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
        _block: &BlockWithTxs,
        _transaction_receipt: &InvokeTransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[MODEL_INDEX])?;
        info!("store delete record: {}", name);

        let model = db.model(&name).await?;

        let mut entity = model.schema().await?;

        db.delete_entity(entity, event_id).await?;
        Ok(())
    }
}
