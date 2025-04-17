use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256 as U256Cainome};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, U256};
use starknet::providers::Provider;
use torii_sqlite::types::ContractType;
use torii_sqlite::Sql;
use tracing::debug;

use crate::EventProcessor;
use crate::EventProcessorConfig;
use crate::task_manager::{TaskId, TaskPriority};
use crate::TaskProcessor;
pub(crate) const LOG_TARGET: &str = "torii::indexer::processors::erc20_legacy_transfer";

#[derive(Default, Debug)]
pub struct Erc20LegacyTransferProcessor;

impl<P> TaskProcessor<P> for Erc20LegacyTransferProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn dependencies(&self) -> Vec<TaskId> {
        vec![]
    }

    fn identifier(&self, event: &Event) -> TaskId {
        let mut hasher = DefaultHasher::new();
        // Hash the contract address
        event.from_address.hash(&mut hasher);
        hasher.finish()
    }
}


#[async_trait]
impl<P> EventProcessor<P> for Erc20LegacyTransferProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn contract_type(&self) -> ContractType {
        ContractType::Erc20
    }

    fn event_key(&self) -> String {
        "Transfer".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // ref: https://github.com/OpenZeppelin/cairo-contracts/blob/1f9359219a92cdb1576f953db71ee993b8ef5f70/src/openzeppelin/token/erc20/library.cairo#L19-L21
        // key: [hash(Transfer)]
        // data: [from, to, value.0, value.1]
        if event.keys.len() == 1 && event.data.len() == 4 {
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
        event_id: &str,
        event: &Event,
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        let token_address = event.from_address;
        let from = event.data[0];
        let to = event.data[1];

        let value = U256Cainome::cairo_deserialize(&event.data, 2)?;
        let value = U256::from_words(value.low, value.high);

        db.handle_erc20_transfer(
            token_address,
            from,
            to,
            value,
            world.provider(),
            block_timestamp,
            event_id,
        )
        .await?;
        debug!(target: LOG_TARGET,from = ?from, to = ?to, value = ?value, "Legacy ERC20 Transfer.");

        Ok(())
    }
}
