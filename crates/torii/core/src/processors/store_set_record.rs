use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;
use torii_client::contract::world::WorldContractReader;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct StoreSetRecordProcessor;

const MODEL_INDEX: usize = 0;
const NUM_KEYS_INDEX: usize = 1;

#[async_trait]
impl<P: Provider + Sync> EventProcessor<P> for StoreSetRecordProcessor {
    fn event_key(&self) -> String {
        "StoreSetRecord".to_string()
    }

    async fn process(
        &self,
        _world: &WorldContractReader<'_, P>,
        db: &mut Sql,
        _provider: &P,
        _block: &BlockWithTxs,
        _transaction_receipt: &InvokeTransactionReceipt,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[MODEL_INDEX])?;
        info!("store set record: {}", name);

        let keys = values_at(&event.data, NUM_KEYS_INDEX)?;
        let values_index = keys.len() + NUM_KEYS_INDEX + 2;
        let values = values_at(&event.data, values_index)?;
        db.set_entity(name, keys, values).await?;
        Ok(())
    }
}

fn values_at(data: &[FieldElement], len_index: usize) -> Result<Vec<FieldElement>, Error> {
    let len: usize = u8::try_from(data[len_index])?.into();
    let start = len_index + 1_usize;
    let end = start + len;
    Ok(data[start..end].to_vec())
}
