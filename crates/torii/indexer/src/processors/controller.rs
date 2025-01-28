use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, Felt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::macros::short_string;
use starknet::providers::Provider;
use torii_sqlite::Sql;
use tracing::{debug, info};

use super::{EventProcessor, EventProcessorConfig};
use crate::task_manager::{TaskId, TaskPriority};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::controller_deployed";

#[derive(Default, Debug)]
pub struct ControllerDeployedProcessor;

#[async_trait]
impl<P> EventProcessor<P> for ControllerDeployedProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "ContractDeployed".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // ContractDeployed event has no keys and contains username in data
        event.keys.is_empty() && !event.data.is_empty()
    }

    fn task_priority(&self) -> TaskPriority {
        3
    }

    fn task_identifier(&self, event: &Event) -> TaskId {
        let mut hasher = DefaultHasher::new();
        event.data[event.data.len() - 1].hash(&mut hasher);
        hasher.finish()
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        block_timestamp: u64,
        event_id: &str,
        event: &Event,
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        // Last felt in data is the salt which is the username encoded as short string
        let username_felt = event.data[event.data.len() - 1];
        let username = parse_cairo_short_string(&username_felt)?;
        // Address is the first felt in data
        let address = event.data[0];

        info!(
            target: LOG_TARGET,
            username = %username,
            address = %address,
            "New controller deployed"
        );

        debug!(
            target: LOG_TARGET,
            username = %username,
            address = %address,
            event_id = %event_id,
            "Processing controller deployment"
        );

        db.add_controller(&username, &format!("{address:#x}"), block_timestamp).await?;

        Ok(())
    }
}
