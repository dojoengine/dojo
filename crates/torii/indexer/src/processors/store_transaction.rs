use std::collections::HashSet;

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
        _transaction_hash: Felt,
        contract_addresses: &HashSet<Felt>,
        transaction: &Transaction,
    ) -> Result<(), Error> {
        db.store_transaction(transaction, block_number, contract_addresses, block_timestamp)?;
        Ok(())
    }
}
