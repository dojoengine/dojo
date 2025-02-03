use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256 as U256Cainome};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, U256};
use starknet::providers::Provider;
use torii_sqlite::Sql;
use tracing::debug;

use super::{EventProcessor, EventProcessorConfig};
use crate::task_manager::{TaskId, TaskPriority};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::erc4906_metadata_update";

#[derive(Default, Debug)]
pub struct Erc4906MetadataUpdateProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc4906MetadataUpdateProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        // We'll handle both event types in validate()
        "MetadataUpdate".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // Single token metadata update: [hash(MetadataUpdate), token_id.low, token_id.high]
        if event.keys.len() == 3 && event.data.is_empty() {
            return true;
        }

        // Batch metadata update: [hash(BatchMetadataUpdate), from_token_id.low, from_token_id.high, to_token_id.low, to_token_id.high]
        if event.keys.len() == 5 && event.data.is_empty() {
            return true;
        }

        false
    }

    fn task_priority(&self) -> TaskPriority {
        2 // Lower priority than transfers
    }

    fn task_identifier(&self, event: &Event) -> TaskId {
        let mut hasher = DefaultHasher::new();
        event.from_address.hash(&mut hasher); // Hash the contract address
        
        // For single token updates
        if event.keys.len() == 3 {
            event.keys[1].hash(&mut hasher); // token_id.low
            event.keys[2].hash(&mut hasher); // token_id.high
        } else {
            // For batch updates, we need to be more conservative
            // Hash just the contract address to serialize all batch updates for the same contract
            // This prevents race conditions with overlapping ranges
        }

        hasher.finish()
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

        if event.keys.len() == 3 {
            // Single token metadata update
            let token_id = U256Cainome::cairo_deserialize(&event.keys, 1)?;
            let token_id = U256::from_words(token_id.low, token_id.high);
            
            db.update_erc721_metadata(token_address, token_id).await?;

            debug!(
                target: LOG_TARGET,
                token_address = ?token_address,
                token_id = ?token_id,
                "ERC721 metadata updated for single token"
            );
        } else {
            // Batch metadata update
            let from_token_id = U256Cainome::cairo_deserialize(&event.keys, 1)?;
            let from_token_id = U256::from_words(from_token_id.low, from_token_id.high);
            
            let to_token_id = U256Cainome::cairo_deserialize(&event.keys, 3)?;
            let to_token_id = U256::from_words(to_token_id.low, to_token_id.high);

            let mut token_id = from_token_id;
            while token_id <= to_token_id {
                db.update_erc721_metadata(token_address, token_id).await?;
                token_id += U256::from(1u8);
            }

            debug!(
                target: LOG_TARGET,
                token_address = ?token_address,
                from_token_id = ?from_token_id,
                to_token_id = ?to_token_id,
                "ERC721 metadata updated for token range"
            );
        }

        Ok(())
    }
}