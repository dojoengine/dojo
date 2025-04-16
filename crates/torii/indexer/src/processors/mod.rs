use std::collections::HashSet;

use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, Felt, Transaction};
use starknet::providers::Provider;
use torii_sqlite::cache::ContractClassCache;
use torii_sqlite::types::Hook;
use torii_sqlite::Sql;

use crate::task_manager::{TaskId, TaskPriority};

pub mod controller;
pub mod erc1155_transfer_batch;
pub mod erc1155_transfer_single;
pub mod erc20_legacy_transfer;
pub mod erc20_transfer;
pub mod erc4906_batch_metadata_update;
pub mod erc4906_metadata_update;
pub mod erc721_legacy_transfer;
pub mod erc721_transfer;
pub mod event_message;
pub mod metadata_update;
pub mod raw_event;
pub mod register_event;
pub mod register_model;
pub mod store_del_record;
pub mod store_set_record;
pub mod store_transaction;
pub mod store_update_member;
pub mod store_update_record;
pub mod upgrade_event;
pub mod upgrade_model;

#[derive(Clone, Debug, Default)]
pub struct EventProcessorConfig {
    pub namespaces: HashSet<String>,
    pub strict_model_reader: bool,
    pub hooks: Vec<Hook>,
}

impl EventProcessorConfig {
    pub fn should_index(&self, namespace: &str) -> bool {
        self.namespaces.is_empty() || self.namespaces.contains(namespace)
    }
}

#[async_trait]
pub trait EventProcessor<P>: Send + Sync
where
    P: Provider + Sync,
{
    fn event_key(&self) -> String;

    fn event_keys_as_string(&self, event: &Event) -> String {
        event.keys.iter().map(|i| format!("{:#064x}", i)).collect::<Vec<_>>().join(",")
    }

    fn validate(&self, event: &Event) -> bool;

    fn task_priority(&self) -> TaskPriority;
    fn task_identifier(&self, event: &Event) -> TaskId;

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
