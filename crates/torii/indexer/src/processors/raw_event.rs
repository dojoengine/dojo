use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::providers::Provider;
use torii_sqlite::Sql;

use super::{EventProcessor, EventProcessorConfig};

#[derive(Default, Debug)]
pub struct RawEventProcessor;

#[async_trait]
impl<P> EventProcessor<P> for RawEventProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "".to_string()
    }

    fn validate(&self, _event: &Event) -> bool {
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        _db: &mut Sql,
        _block_number: u64,
        _block_timestamp: u64,
        _event_id: &str,
        _event: &Event,
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        // We can choose to consider them, or not.

        Ok(())
    }
}
