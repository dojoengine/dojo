use anyhow::{Error, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt, TransactionReceipt};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};

use crate::State;

pub mod register_component;
pub mod register_system;
pub mod store_set_record;
pub mod store_system_call;

#[async_trait]
pub trait EventProcessor<S: State, T: JsonRpcTransport> {
    fn event_key(&self) -> String;
    async fn process(
        &self,
        storage: &S,
        provider: &JsonRpcClient<T>,
        block: &BlockWithTxs,
        invoke_receipt: &InvokeTransactionReceipt,
        event: &Event,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait BlockProcessor<S: State, T: JsonRpcTransport> {
    fn get_block_number(&self) -> String;
    async fn process(
        &self,
        storage: &S,
        provider: &JsonRpcClient<T>,
        block: &BlockWithTxs,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait TransactionProcessor<S: State, T: JsonRpcTransport> {
    async fn process(
        &self,
        storage: &S,
        provider: &JsonRpcClient<T>,
        block: &BlockWithTxs,
        transaction_receipt: &TransactionReceipt,
    ) -> Result<(), Error>;
}
