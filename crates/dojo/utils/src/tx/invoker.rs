//! Invoker to invoke contracts.

use starknet::accounts::ConnectedAccount;
use starknet::core::types::Call;
use tracing::trace;

use super::TransactionResult;
use crate::tx::FeeConfig;
use crate::{TransactionError, TransactionExt, TransactionWaiter, TxnConfig};

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

    /// Extends the calls to the invoker.
    pub fn extend_calls(&mut self, calls: Vec<Call>) {
        self.calls.extend(calls);
    }

    /// Clean all the calls of the invoker.
    pub fn clean_calls(&mut self) {
        self.calls.clear();
    }

    /// First uses the ordered calls, and then extends with the
    /// calls already added (considered as non-ordered).
    pub fn extends_ordered(&mut self, ordered_calls: Vec<Call>) {
        for call in ordered_calls.into_iter().rev() {
            self.calls.insert(0, call);
        }
    }

    /// Invokes a single call.
    pub async fn invoke(
        &self,
        call: Call,
    ) -> Result<TransactionResult, TransactionError<A::SignError>> {
        trace!(?call, "Invoke contract.");

        let tx = match self.txn_config.fee_config {
            FeeConfig::Strk(config) => {
                trace!(?config, "Invoking with STRK.");
                self.account.execute_v3(vec![call]).send_with_cfg(&self.txn_config).await?
            }
            FeeConfig::Eth(config) => {
                trace!(?config, "Invoking with ETH.");
                self.account.execute_v1(vec![call]).send_with_cfg(&self.txn_config).await?
            }
        };

        trace!(transaction_hash = format!("{:#066x}", tx.transaction_hash), "Invoke contract.");

        if self.txn_config.wait {
            let receipt =
                TransactionWaiter::new(tx.transaction_hash, &self.account.provider()).await?;

            if self.txn_config.receipt {
                return Ok(TransactionResult::HashReceipt(tx.transaction_hash, Box::new(receipt)));
            }
        }

        Ok(TransactionResult::Hash(tx.transaction_hash))
    }

    /// Invokes all the calls in one single transaction.
    pub async fn multicall(&self) -> Result<TransactionResult, TransactionError<A::SignError>> {
        if self.calls.is_empty() {
            return Ok(TransactionResult::Noop);
        }

        trace!(?self.calls, "Invoke contract multicall.");

        let tx = match self.txn_config.fee_config {
            FeeConfig::Strk(config) => {
                trace!(?config, "Invoking with STRK.");
                self.account.execute_v3(self.calls.clone()).send_with_cfg(&self.txn_config).await?
            }
            FeeConfig::Eth(config) => {
                trace!(?config, "Invoking with ETH.");
                self.account.execute_v1(self.calls.clone()).send_with_cfg(&self.txn_config).await?
            }
        };

        trace!(
            transaction_hash = format!("{:#066x}", tx.transaction_hash),
            "Invoke contract multicall."
        );

        if self.txn_config.wait {
            let receipt =
                TransactionWaiter::new(tx.transaction_hash, &self.account.provider()).await?;

            if self.txn_config.receipt {
                return Ok(TransactionResult::HashReceipt(tx.transaction_hash, Box::new(receipt)));
            }
        }

        Ok(TransactionResult::Hash(tx.transaction_hash))
    }

    /// Invokes all the calls individually, usually used for debugging if a multicall failed.
    ///
    /// The order of the calls is the same as the order of the calls added to the invoker.
    pub async fn invoke_all_sequentially(
        &self,
    ) -> Result<Vec<TransactionResult>, TransactionError<A::SignError>> {
        if !self.calls.is_empty() {
            let mut results = vec![];

            for call in self.calls.iter() {
                results.push(self.invoke(call.clone()).await?);
            }

            return Ok(results);
        }

        Ok(vec![])
    }
}
