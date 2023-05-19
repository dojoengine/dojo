use anyhow::{Error, Result};
use apibara_core::starknet::v1alpha2::{Block, EventWithTransaction, TransactionWithReceipt};
use sqlx::{Pool, Sqlite};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tonic::async_trait;

pub mod component_register;
pub mod component_state_update;
pub mod system_register;

#[async_trait]
pub trait EventProcessor<S: JsonRpcTransport> {
    fn event_key(&self) -> String;
    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<S>,
        data: EventWithTransaction,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait BlockProcessor<S: JsonRpcTransport> {
    fn get_block_number(&self) -> String;
    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<S>,
        data: Block,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait TransactionProcessor<S: JsonRpcTransport> {
    fn get_transaction_hash(&self) -> String;
    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<S>,
        data: TransactionWithReceipt,
    ) -> Result<(), Error>;
}
