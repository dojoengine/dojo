use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockWithTxs, Event, TransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet_crypto::FieldElement;
use tracing::info;

use super::EventProcessor;
use crate::state::State;

#[derive(Default)]
pub struct StoreSetRecordProcessor;

const COMPONENT_INDEX: usize = 0;
const NUM_KEYS_INDEX: usize = 1;

#[async_trait]
impl<S: State + Sync, T: JsonRpcTransport> EventProcessor<S, T> for StoreSetRecordProcessor {
    fn event_key(&self) -> String {
        "StoreSetRecord".to_string()
    }

    async fn process(
        &self,
        storage: &S,
        _provider: &JsonRpcClient<T>,
        _block: &BlockWithTxs,
        _transaction_receipt: &TransactionReceipt,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[COMPONENT_INDEX])?;
        info!("store set record: {}", name);

        let keys = values_at(&event.data, NUM_KEYS_INDEX)?;
        let values_index = keys.len() + NUM_KEYS_INDEX + 2;
        let values = values_at(&event.data, values_index)?;
        // TODO: are we removing partitions?
        let partition = FieldElement::ZERO;

        storage.set_entity(name, partition, keys, values).await?;
        Ok(())
    }
}

fn values_at(data: &[FieldElement], len_index: usize) -> Result<Vec<FieldElement>, Error> {
    let len: usize = u8::try_from(data[len_index])?.into();
    let start = len_index + 1_usize;
    let end = start + len;
    Ok(data[start..end].to_vec())
}
