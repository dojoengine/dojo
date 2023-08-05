use anyhow::{Error, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockId, BlockWithTxs, Event, StateDiff, TransactionReceipt};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet_crypto::FieldElement;

use crate::state::State;

pub mod register_component;
pub mod register_system;
pub mod store_set_record;
pub mod store_set_record_state_diff;

#[async_trait]
pub trait StateDiffProcessor<S: State> {
    async fn process(
        &self,
        storage: &S,
        component: String,
        world: FieldElement,
        length: usize,
        keys: Vec<FieldElement>,
        state_diff: &StateDiff,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait EventProcessor<S: State, T: JsonRpcTransport> {
    fn event_key(&self) -> String;
    async fn process(
        &self,
        storage: &S,
        provider: &JsonRpcClient<T>,
        block: &BlockWithTxs,
        transaction_receipt: &TransactionReceipt,
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
    fn get_transaction_hash(&self) -> String;
    async fn process(
        &self,
        storage: &S,
        provider: &JsonRpcClient<T>,
        block: &BlockWithTxs,
        transaction_receipt: &TransactionReceipt,
    ) -> Result<(), Error>;
}
