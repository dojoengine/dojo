use anyhow::{Error, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt, TransactionReceipt};
use starknet::providers::Provider;
use torii_client::contract::world::WorldContractReader;

use crate::sql::Sql;

pub mod register_model;
pub mod register_system;
pub mod store_set_record;
// pub mod store_system_call;

#[async_trait]
pub trait EventProcessor<P: Provider + Sync> {
    fn event_key(&self) -> String;
    async fn process(
        &self,
        world: &WorldContractReader<'_, P>,
        db: &mut Sql,
        provider: &P,
        invoke_receipt: &InvokeTransactionReceipt,
        event: &Event,
        event_idx: usize,
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
        transaction_receipt: &TransactionReceipt,
    ) -> Result<(), Error>;
}
