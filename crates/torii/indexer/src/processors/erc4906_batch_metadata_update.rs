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

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::erc4906_metadata_update_batch";

#[derive(Default, Debug)]
pub struct Erc4906BatchMetadataUpdateProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc4906BatchMetadataUpdateProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "BatchMetadataUpdate".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // Batch metadata update: [hash(BatchMetadataUpdate), from_token_id.low, from_token_id.high,
        // to_token_id.low, to_token_id.high]
        event.keys.len() == 5 && event.data.is_empty()
    }

    fn task_priority(&self) -> TaskPriority {
        2
    }

    fn task_identifier(&self, _event: &Event) -> TaskId {
        task_manager::TASK_ID_SEQUENTIAL
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        _block_timestamp: u64,
        _event_id: &str,
        event: &Event,
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        let token_address = event.from_address;
        let from_token_id = U256Cainome::cairo_deserialize(&event.keys, 1)?;
        let from_token_id = U256::from_words(from_token_id.low, from_token_id.high);

        let to_token_id = U256Cainome::cairo_deserialize(&event.keys, 3)?;
        let to_token_id = U256::from_words(to_token_id.low, to_token_id.high);

        let mut token_id = from_token_id;
        while token_id <= to_token_id {
            db.update_nft_metadata(token_address, token_id).await?;
            token_id += U256::from(1u8);
        }

        debug!(
            target: LOG_TARGET,
            token_address = ?token_address,
            from_token_id = ?from_token_id,
            to_token_id = ?to_token_id,
            "NFT metadata updated for token range"
        );

        Ok(())
    }
}
