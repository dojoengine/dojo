use anyhow::{Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt, InvokeTransactionV1};
use starknet::providers::Provider;

use crate::sql::Sql;

pub mod metadata_update;
pub mod register_model;
pub mod store_set_record;
pub mod store_transaction;

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
        block: &BlockWithTxs,
        invoke_receipt: &InvokeTransactionReceipt,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait BlockProcessor<P: Provider + Sync> {
    fn get_block_number(&self) -> String;
    async fn process(&self, db: &mut Sql, provider: &P, block: &BlockWithTxs) -> Result<(), Error>;
}

#[async_trait]
pub trait TransactionProcessor<P: Provider + Sync> {
    async fn process(
        &self,
        db: &mut Sql,
        provider: &P,
        block: &BlockWithTxs,
        invoke_receipt: &InvokeTransactionReceipt,
        transaction: &InvokeTransactionV1,
        transaction_id: &str,
    ) -> Result<(), Error>;
}
