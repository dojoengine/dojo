use anyhow::{Error, Ok, Result};
use apibara_client_protos::pb::starknet::v1alpha2::{
    Block, Event, EventWithTransaction, FieldElement, Transaction, TransactionWithReceipt,
};
use tonic::async_trait;

use crate::prisma;

pub mod component_register;
pub mod component_state_update;
mod system_register;

#[async_trait]
pub trait IProcessor<T> {
    async fn process(&self, client: &prisma::PrismaClient, data: T) -> Result<(), Error>;
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
