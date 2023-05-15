use anyhow::{Error, Result};
use starknet::providers::jsonrpc::models::{BlockWithTxs, Event, Transaction};
use tonic::async_trait;

pub mod component_register;
pub mod component_state_update;
pub mod system_register;

#[async_trait]
pub trait BlockProcessor {
    fn get_block_number(&self) -> String;
    async fn process(&self, block: BlockWithTxs) -> Result<(), Error>;
}

#[async_trait]
pub trait TransactionProcessor {
    fn get_transaction_hash(&self) -> String;
    async fn process(&self, block: BlockWithTxs, transaction: Transaction) -> Result<(), Error>;
}

#[async_trait]
pub trait EventProcessor {
    fn event_key(&self) -> String;
    async fn process(
        &self,
        block: BlockWithTxs,
        transaction: Transaction,
        event: Event,
    ) -> Result<(), Error>;
}
