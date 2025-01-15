use std::hash::{DefaultHasher, Hash, Hasher};
use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256 as U256Cainome};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, U256};
use starknet::providers::Provider;
use torii_sqlite::Sql;
use tracing::debug;

use crate::task_manager::TaskId;

use super::{EventProcessor, EventProcessorConfig};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::erc721_transfer";

#[derive(Default, Debug)]
pub struct Erc721TransferProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc721TransferProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "Transfer".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // ref: https://github.com/OpenZeppelin/cairo-contracts/blob/ba00ce76a93dcf25c081ab2698da20690b5a1cfb/packages/token/src/erc721/erc721.cairo#L40-L49
        // key: [hash(Transfer), from, to, token_id.low, token_id.high]
        // data: []
        if event.keys.len() == 5 && event.data.is_empty() {
            return true;
        }

        false
    }

    fn task_priority(&self) -> usize {
        1
    }

    fn task_identifier(&self, event: &Event) -> TaskId {
        let mut hasher = DefaultHasher::new();
        // Hash the event key (Transfer)
        event.keys[0].hash(&mut hasher);

        // Take the max of from/to addresses to get a canonical representation
        // This ensures transfers between the same pair of addresses are grouped together
        // regardless of direction (A->B or B->A)
        let canonical_pair = std::cmp::max(event.keys[1], event.keys[2]); 
        canonical_pair.hash(&mut hasher);

        // For ERC721, we can safely parallelize by token ID since each token is unique
        // and can only be owned by one address at a time. This means:
        // 1. Transfers of different tokens can happen in parallel
        // 2. Multiple transfers of the same token must be sequential
        // 3. The canonical address pair ensures related transfers stay together
        event.keys[3].hash(&mut hasher);
        event.keys[4].hash(&mut hasher);
        
        hasher.finish()
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
        let from = event.keys[1];
        let to = event.keys[2];

        let token_id = U256Cainome::cairo_deserialize(&event.keys, 3)?;
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
