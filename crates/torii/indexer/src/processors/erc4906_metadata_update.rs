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

pub(crate) const LOG_TARGET: &str = "torii::indexer::processors::erc4906_metadata_update";
#[derive(Default, Debug)]
pub struct Erc4906MetadataUpdateProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc4906MetadataUpdateProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "MetadataUpdate".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // Single token metadata update: [hash(MetadataUpdate), token_id.low, token_id.high]
        event.keys.len() == 3 && event.data.is_empty()
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
        let token_id = U256Cainome::cairo_deserialize(&event.keys, 1)?;
        let token_id = U256::from_words(token_id.low, token_id.high);

        db.update_nft_metadata(token_address, token_id).await?;

        debug!(
            target: LOG_TARGET,
            token_address = ?token_address,
            token_id = ?token_id,
            "NFT metadata updated for single token"
        );

        Ok(())
    }
}
