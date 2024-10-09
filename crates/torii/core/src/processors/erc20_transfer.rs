use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256 as U256Cainome};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, U256};
use starknet::providers::Provider;
use tracing::debug;

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
        // ref: https://github.com/OpenZeppelin/cairo-contracts/blob/ba00ce76a93dcf25c081ab2698da20690b5a1cfb/packages/token/src/erc20/erc20.cairo#L38-L46
        // key: [hash(Transfer), from, to]
        // data: [value.0, value.1]
        if event.keys.len() == 3 && event.data.len() == 2 {
            return true;
        }

        false
    }

    async fn process(
        &self,
        world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        block_timestamp: u64,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let token_address = event.from_address;
        let from = event.keys[1];
        let to = event.keys[2];

        let value = U256Cainome::cairo_deserialize(&event.data, 0)?;
        let value = U256::from_words(value.low, value.high);

        db.handle_erc20_transfer(token_address, from, to, value, world.provider(), block_timestamp)
            .await?;
        debug!(target: LOG_TARGET,from = ?from, to = ?to, value = ?value, "ERC20 Transfer");

        Ok(())
    }
}
