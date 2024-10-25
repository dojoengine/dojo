//! Invoker to invoke contracts.

use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::Call;
use tracing::trace;

use super::MigrationError;

#[derive(Debug)]
pub struct Invoker<A>
where
    A: ConnectedAccount + Send + Sync,
{
    /// The account to use to deploy the contracts.
    pub account: A,
    /// The transaction configuration.
    pub txn_config: TxnConfig,
    /// The calls to invoke.
    pub calls: Vec<Call>,
}

impl<A> Invoker<A>
where
    A: ConnectedAccount + Send + Sync,
{
    /// Creates a new invoker.
    pub fn new(account: A, txn_config: TxnConfig) -> Self {
        Self { account, txn_config, calls: vec![] }
    }

    /// Adds a call to the invoker.
    pub fn add_call(&mut self, call: Call) {
        self.calls.push(call);
    }

    /// Invokes a single call.
    pub async fn invoke(&self, call: Call) -> Result<(), MigrationError<A::SignError>> {
        trace!(?call, "Invoke contract.");

        let tx = self.account.execute_v1(vec![call]).send_with_cfg(&self.txn_config).await?;

        trace!(
            transaction_hash = format!("{:#066x}", tx.transaction_hash),
            "Invoke contract."
        );

        if self.txn_config.wait {
            TransactionWaiter::new(tx.transaction_hash, &self.account.provider()).await?;
        }

        Ok(())
    }

    /// Invokes all the calls in one single transaction.
    pub async fn multicall(&self) -> Result<(), MigrationError<A::SignError>> {
        if self.calls.is_empty() {
            return Ok(());
        }

        trace!(?self.calls, "Invoke contract multicall.");

        let tx = self.account.execute_v1(self.calls.clone()).send_with_cfg(&self.txn_config).await?;

        trace!(
            transaction_hash = format!("{:#066x}", tx.transaction_hash),
            "Invoke contract multicall."
        );

        if self.txn_config.wait {
            TransactionWaiter::new(tx.transaction_hash, &self.account.provider()).await?;
        }

        Ok(())
    }

    /// Invokes all the calls individually, usually used for debugging if a multicall failed.
    ///
    /// The order of the calls is the same as the order of the calls added to the invoker.
    pub async fn invoke_all_sequentially(&self) -> Result<(), MigrationError<A::SignError>> {
        if !self.calls.is_empty() {
            for call in self.calls.iter() {
                self.invoke(call.clone()).await?;
            }
        }

        Ok(())
    }
}
