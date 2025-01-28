use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::{core::types::Event, macros::felt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use starknet_crypto::Felt;
use torii_sqlite::Sql;
use tracing::{debug, info};

use super::{EventProcessor, EventProcessorConfig};
use crate::task_manager::{TaskId, TaskPriority};

pub(crate) const LOG_TARGET: &str = "torii_indexer::processors::controller";

pub(crate) const CARTRIDGE_PAYMASTER_EUWEST3_ADDRESS: Felt =
    felt!("0x359a81f67140632ec91c7f9af3fc0b5bca0a898ae0be3f7682585b0f40119a7");
pub(crate) const CARTRIDGE_PAYMASTER_SEA1_ADDRESS: Felt =
    felt!("0x07a0f23c43a291282d093e85f7fb7c0e23a66d02c10fead324ce4c3d56c4bd67");
pub(crate) const CARTRIDGE_PAYMASTER_USEAST4_ADDRESS: Felt =
    felt!("0x2d2e564dd4faa14277fefd0d8cb95e83b13c0353170eb6819ec35bf1bee8e2a");

#[derive(Default, Debug)]
pub struct ControllerProcessor;

#[async_trait]
impl<P> EventProcessor<P> for ControllerProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "ContractDeployed".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // ContractDeployed event has no keys and contains username in data
        event.keys.len() == 1 && !event.data.is_empty()
    }

    fn task_priority(&self) -> TaskPriority {
        3
    }

    fn task_identifier(&self, event: &Event) -> TaskId {
        let mut hasher = DefaultHasher::new();
        // the contract address is the first felt in data
        event.data[0].hash(&mut hasher);
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
        // Address is the first felt in data
        let address = event.data[0];
        // Deployer is the second felt in data
        let deployer = event.data[1];

        // Check if the deployer is a cartridge paymaster
        if !CARTRIDGE_PAYMASTER_EUWEST3_ADDRESS.eq(&deployer)
            && !CARTRIDGE_PAYMASTER_SEA1_ADDRESS.eq(&deployer)
            && !CARTRIDGE_PAYMASTER_USEAST4_ADDRESS.eq(&deployer)
        {
            // ignore non-cartridge controller deployments
            return Ok(());
        }

        // Last felt in data is the salt which is the username encoded as short string
        let username_felt = event.data[event.data.len() - 1];
        let username = parse_cairo_short_string(&username_felt)?;

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
