use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{Felt, Transaction};
use starknet::providers::Provider;
use torii_sqlite::Sql;

use super::TransactionProcessor;

#[derive(Default, Debug)]
pub struct StoreTransactionProcessor;

#[async_trait]
impl<P: Provider + Sync + std::fmt::Debug> TransactionProcessor<P> for StoreTransactionProcessor {
    async fn process(
        &self,
        db: &mut Sql,
        _provider: &P,
        block_number: u64,
        block_timestamp: u64,
        transaction_hash: Felt,
        transaction: &Transaction,
    ) -> Result<(), Error> {
        let transaction_id = format!("{:#064x}:{:#x}", block_number, transaction_hash);
        db.store_transaction(transaction, &transaction_id, block_timestamp)?;
        Ok(())
    }
}
