use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256 as U256Cainome};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, U256};
use starknet::providers::Provider;
use tracing::debug;

use super::{EventProcessor, EventProcessorConfig};
use torii_sqlite::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::erc721_legacy_transfer";

#[derive(Default, Debug)]
pub struct Erc721LegacyTransferProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc721LegacyTransferProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "Transfer".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // ref: https://github.com/OpenZeppelin/cairo-contracts/blob/1f9359219a92cdb1576f953db71ee993b8ef5f70/src/openzeppelin/token/erc721/library.cairo#L27-L29
        // key: [hash(Transfer)]
        // data: [from, to, token_id.0, token_id.1]
        if event.keys.len() == 1 && event.data.len() == 4 {
            return true;
        }

        false
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        block_number: u64,
        block_timestamp: u64,
        event_id: &str,
        event: &Event,
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        let token_address = event.from_address;
        let from = event.data[0];
        let to = event.data[1];

        let token_id = U256Cainome::cairo_deserialize(&event.data, 2)?;
        let token_id = U256::from_words(token_id.low, token_id.high);

        db.handle_erc721_transfer(
            token_address,
            from,
            to,
            token_id,
            block_timestamp,
            event_id,
            block_number,
        )
        .await?;
        debug!(target: LOG_TARGET, from = ?from, to = ?to, token_id = ?token_id, "ERC721 Transfer");

        Ok(())
    }
}
