use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, Transaction, TransactionReceipt};
use starknet::providers::Provider;
use starknet_crypto::FieldElement;

use crate::sql::Sql;

pub mod event_message;
pub mod metadata_update;
pub mod register_model;
pub mod store_del_record;
pub mod store_set_record;
pub mod store_transaction;

const MODEL_INDEX: usize = 0;
const NUM_KEYS_INDEX: usize = 1;

#[async_trait]
pub trait EventProcessor<P>
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
        transaction_receipt: &TransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait BlockProcessor<P: Provider + Sync> {
    fn get_block_number(&self) -> String;
    async fn process(
        &self,
        db: &mut Sql,
        provider: &P,
        block_number: u64,
        block_timestamp: u64,
        block_hash: FieldElement,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait TransactionProcessor<P: Provider + Sync> {
    #[allow(clippy::too_many_arguments)]
    async fn process(
        &self,
        db: &mut Sql,
        provider: &P,
        block_number: u64,
        block_timestamp: u64,
        transaction_receipt: &TransactionReceipt,
        transaction_hash: FieldElement,
        transaction: &Transaction,
    ) -> Result<(), Error>;
}
