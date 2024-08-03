use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, TransactionReceiptWithBlockInfo};
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::erc20_transfer";

#[derive(Default, Debug)]
pub struct Erc20TransferProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc20TransferProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "Transfer".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() == 3 {
            info!(
                target: LOG_TARGET,
                event_key = %<Erc20TransferProcessor as EventProcessor<P>>::event_key(self),
                invalid_keys = %<Erc20TransferProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
                "Invalid event keys."
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        _db: &mut Sql,
        _block_number: u64,
        _block_timestamp: u64,
        _transaction_receipt: &TransactionReceiptWithBlockInfo,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let from = event.keys[1];
        let to = event.keys[2];

        let value = U256::cairo_deserialize(&event.data, 0)?;
        println!("from: {:?}, to: {:?}, value: {:?}", from, to, value);

        Ok(())
    }
}
