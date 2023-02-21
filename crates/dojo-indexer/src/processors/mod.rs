use anyhow::{Result, Error, Ok};
use apibara_client_protos::pb::starknet::v1alpha2::{Event, Block, Transaction, EventWithTransaction, FieldElement};
use tonic::async_trait;

use crate::prisma;

pub mod component_state_update;
pub mod component_register;
mod system_register;

#[async_trait]
pub trait IProcessor<T> {
    async fn process(&self, client: &prisma::PrismaClient, data: T) -> Result<(), Error>;
}

#[async_trait]
pub trait EventProcessor: IProcessor<EventWithTransaction> {
    fn get_event_key(&self) -> String;
}

pub trait BlockProcessor: IProcessor<Block> {
    fn get_block_number(&self) -> String;
}

pub trait TransactionProcessor: IProcessor<Transaction> {
    fn get_transaction_hash(&self) -> String;
}