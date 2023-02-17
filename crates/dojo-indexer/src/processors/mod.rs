use anyhow::{Result, Error, Ok};
use apibara_client_protos::pb::starknet::v1alpha2::{Event, Block, Transaction};
use tonic::async_trait;

use crate::prisma;

pub mod component_state_update;
pub mod component_register;
mod system_register;

#[async_trait]
pub trait IProcessor<T> {
    async fn process(&self, client: &prisma::PrismaClient, data: T) -> Result<(), Error>;
}

pub struct EventProcessor;
impl EventProcessor {
    fn new() -> Self {
        Self {}
    }
}
#[async_trait]
impl IProcessor<Event> for EventProcessor {
    async fn process(&self, client: &prisma::PrismaClient, data: Event) -> Result<(), Error> {
        Ok(())
    }
}

pub struct BlockProcessor;
impl BlockProcessor {
    fn new() -> Self {
        Self {}
    }
}
#[async_trait]
impl IProcessor<Block> for BlockProcessor {
    async fn process(&self, client: &prisma::PrismaClient, data: Block) -> Result<(), Error> {
        Ok(())
    }
}

pub struct TransactionProcessor;
impl TransactionProcessor {
    fn new() -> Self {
        Self {}
    }
}
#[async_trait]
impl IProcessor<Transaction> for TransactionProcessor {
    async fn process(&self, client: &prisma::PrismaClient, data: Transaction) -> Result<(), Error> {
        Ok(())
    }
}