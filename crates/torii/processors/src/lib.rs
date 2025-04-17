use std::collections::HashSet;

use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, Felt, Transaction};
use starknet::providers::Provider;
use task_manager::TaskId;
use torii_sqlite::cache::ContractClassCache;
use torii_sqlite::types::ContractType;
use torii_sqlite::Sql;

mod task_manager;
mod processors;

#[derive(Clone, Debug, Default)]
pub struct EventProcessorConfig {
    pub namespaces: HashSet<String>,
    pub strict_model_reader: bool,
}

impl EventProcessorConfig {
    pub fn should_index(&self, namespace: &str) -> bool {
        self.namespaces.is_empty() || self.namespaces.contains(namespace)
    }
}

pub trait TaskProcessor<P>: Send + Sync
where
    P: Provider + Sync,
{
    fn dependencies(&self) -> Vec<TaskId>;
    fn identifier(&self, event: &Event) -> TaskId;
}

#[async_trait]
pub trait EventProcessor<P>: Send + Sync
where
    P: Provider + Sync,
{
    fn contract_type(&self) -> ContractType;
    fn event_key(&self) -> String;

    fn event_keys_as_string(&self, event: &Event) -> String {
        event.keys.iter().map(|i| format!("{:#064x}", i)).collect::<Vec<_>>().join(",")
    }

    fn validate(&self, event: &Event) -> bool;

    #[allow(clippy::too_many_arguments)]
    async fn process(
        &self,
        world: &WorldContractReader<P>,
        db: &mut Sql,
        block_number: u64,
        block_timestamp: u64,
        event_id: &str,
        event: &Event,
        _config: &EventProcessorConfig,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait BlockProcessor<P: Provider + Sync>: Send + Sync {
    fn get_block_number(&self) -> String;
    async fn process(
        &self,
        db: &mut Sql,
        provider: &P,
        block_number: u64,
        block_timestamp: u64,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait TransactionProcessor<P: Provider + Sync + std::fmt::Debug>: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    async fn process(
        &self,
        db: &mut Sql,
        provider: &P,
        block_number: u64,
        block_timestamp: u64,
        transaction_hash: Felt,
        contract_addresses: &HashSet<Felt>,
        transaction: &Transaction,
        contract_class_cache: &ContractClassCache<P>,
    ) -> Result<(), Error>;
}
