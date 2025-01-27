use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256 as U256Cainome};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, U256};
use starknet::providers::Provider;
use torii_sqlite::Sql;
use tracing::debug;

use super::{EventProcessor, EventProcessorConfig};
use crate::task_manager::{self, TaskId, TaskPriority};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::erc1155_transfer_single";

#[derive(Default, Debug)]
pub struct Erc1155TransferSingleProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc1155TransferSingleProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "TransferSingle".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // key: [hash(TransferSingle), operator, from, to]
        // data: [id.low, id.high, value.low, value.high]
        if event.keys.len() == 4 && event.data.len() == 4 {
            return true;
        }
        false
    }

    fn task_priority(&self) -> TaskPriority {
        1
    }

    fn task_identifier(&self, _event: &Event) -> TaskId {
        task_manager::TASK_ID_SEQUENTIAL
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
        let from = event.keys[2];
        let to = event.keys[3];

        let token_id = U256Cainome::cairo_deserialize(&event.data, 0)?;
        let token_id = U256::from_words(token_id.low, token_id.high);

        let amount = U256Cainome::cairo_deserialize(&event.data, 2)?;
        let amount = U256::from_words(amount.low, amount.high);

        db.handle_nft_transfer(
            token_address,
            from,
            to,
            token_id,
            amount,
            block_timestamp,
            event_id,
            block_number,
        )
        .await?;
        debug!(target: LOG_TARGET, from = ?from, to = ?to, token_id = ?token_id, amount = ?amount, "ERC1155 TransferSingle");

        Ok(())
    }
}
