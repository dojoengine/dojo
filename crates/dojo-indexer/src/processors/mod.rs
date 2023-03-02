use anyhow::{Error, Result};
use apibara_client_protos::pb::starknet::v1alpha2::{
    Block, EventWithTransaction, TransactionWithReceipt,
};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use tonic::async_trait;

use crate::prisma;

pub mod component_register;
pub mod component_state_update;
pub mod system_register;

#[async_trait]
pub trait IProcessor<T> {
    async fn process(
        &self,
        client: &prisma::PrismaClient,
        provider: &JsonRpcClient<HttpTransport>,
        data: T,
    ) -> Result<(), Error>;
}

pub trait EventProcessor: IProcessor<EventWithTransaction> {
    fn get_event_key(&self) -> String;
}

pub trait BlockProcessor: IProcessor<Block> {
    fn get_block_number(&self) -> String;
}

pub trait TransactionProcessor: IProcessor<TransactionWithReceipt> {
    fn get_transaction_hash(&self) -> String;
}
