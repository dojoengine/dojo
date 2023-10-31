use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;
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
        world: &WorldContractReader<P>,
        db: &mut Sql,
        _block: &BlockWithTxs,
        _transaction_receipt: &InvokeTransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[MODEL_INDEX])?;
        info!("store set record: {}", name);

        let model = world.model(&name).await?;
        let keys = values_at(&event.data, NUM_KEYS_INDEX)?;
        let entity = model.entity(&keys).await?;
        db.set_entity(entity, event_id).await?;
        Ok(())
    }
}

fn values_at(data: &[FieldElement], len_index: usize) -> Result<Vec<FieldElement>, Error> {
    let len: usize = u8::try_from(data[len_index])?.into();
    let start = len_index + 1_usize;
    let end = start + len;
    Ok(data[start..end].to_vec())
}
