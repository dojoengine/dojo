use anyhow::{Error, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite};
use starknet::core::types::{BlockWithTxs, Event, TransactionReceipt};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};

// pub mod component_register;
// pub mod component_state_update;
// pub mod system_register;

#[async_trait]
pub trait EventProcessor<S: JsonRpcTransport> {
    fn event_key(&self) -> String;
    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<S>,
        event: &Event,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait BlockProcessor<S: JsonRpcTransport> {
    fn get_block_number(&self) -> String;
    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<S>,
        block: &BlockWithTxs,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait TransactionProcessor<S: JsonRpcTransport> {
    fn get_transaction_hash(&self) -> String;
    async fn process(
        &self,
        pool: &Pool<Sqlite>,
        provider: &JsonRpcClient<S>,
        transaction_receipt: &TransactionReceipt,
    ) -> Result<(), Error>;
}
