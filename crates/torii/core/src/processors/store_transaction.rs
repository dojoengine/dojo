use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockWithTxs, Transaction, TransactionReceipt};
use starknet::providers::Provider;

use super::TransactionProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct StoreTransactionProcessor;

#[async_trait]
impl<P: Provider + Sync> TransactionProcessor<P> for StoreTransactionProcessor {
    async fn process(
        &self,
        db: &mut Sql,
        _provider: &P,
        _block: &BlockWithTxs,
        _receipt: &TransactionReceipt,
        transaction: &Transaction,
        transaction_id: &str,
    ) -> Result<(), Error> {
        db.store_transaction(transaction, transaction_id);

        Ok(())
    }
}
