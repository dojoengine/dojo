use std::collections::HashMap;

use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, Felt, Transaction};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;

use crate::sql::Sql;

// pub mod erc20_legacy_transfer;
// pub mod erc20_transfer;
// pub mod erc721_transfer;
pub mod erc20_legacy_transfer;
pub mod erc20_transfer;
pub mod erc721_transfer;
pub mod event_message;
pub mod metadata_update;
pub mod register_model;
pub mod store_del_record;
pub mod store_set_record;
pub mod store_transaction;
pub mod store_update_member;
pub mod store_update_record;

const MODEL_INDEX: usize = 0;
const ENTITY_ID_INDEX: usize = 1;
const NUM_KEYS_INDEX: usize = 2;

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

    #[allow(clippy::too_many_arguments)]
    async fn process(
        &self,
        world: &WorldContractReader<P>,
        db: &mut Sql,
        block_number: u64,
        block_timestamp: u64,
        event_id: &str,
        event: &Event,
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
pub trait TransactionProcessor<P: Provider + Sync>: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    async fn process(
        &self,
        db: &mut Sql,
        provider: &P,
        block_number: u64,
        block_timestamp: u64,
        transaction_hash: Felt,
        transaction: &Transaction,
    ) -> Result<(), Error>;
}

type EventProcessors<P> = Vec<Box<dyn EventProcessor<P>>>;

/// Given a list of event processors, generate a map of event keys to the event processor
pub fn generate_event_processors_map<P: Provider + Sync + Send>(
    event_processor: EventProcessors<P>,
) -> Result<HashMap<Felt, EventProcessors<P>>> {
    let mut event_processors = HashMap::new();

    for processor in event_processor {
        let key = get_selector_from_name(processor.event_key().as_str())?;
        event_processors.entry(key).or_insert(vec![]).push(processor);
    }

    Ok(event_processors)
}
