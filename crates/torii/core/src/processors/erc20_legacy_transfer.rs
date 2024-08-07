use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256 as U256Cainome};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, TransactionReceiptWithBlockInfo, U256};
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::erc20_legacy_transfer";

#[derive(Default, Debug)]
pub struct Erc20LegacyTransferProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc20LegacyTransferProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "Transfer".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // key: [hash(Transfer)]
        // data: [from, to, value.0, value.1]
        if event.keys.len() == 1 && event.data.len() == 4 {
            return true;
        }

        false
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
        let token_address = event.from_address;
        let from = event.data[0];
        let to = event.data[1];

        let value = U256Cainome::cairo_deserialize(&event.data, 2)?;
        let value = U256::from_words(value.low, value.high);

        db.handle_erc20_transfer(token_address, from, to, value).await?;
        info!(target: LOG_TARGET,from = ?from, to = ?to, value = ?value, "Legacy ERC20 Transfer");

        Ok(())
    }
}