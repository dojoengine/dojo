mod waiter;

pub use waiter::*;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::Result;
use futures::FutureExt;
use starknet::accounts::{
    AccountDeploymentV1, AccountError, AccountFactory, AccountFactoryError, ConnectedAccount,
    DeclarationV2, ExecutionV1,
};
use starknet::core::types::{
    DeclareTransactionResult, DeployAccountTransactionResult, ExecutionResult, Felt,
    InvokeTransactionResult, ReceiptBlock, StarknetError, TransactionFinalityStatus,
    TransactionReceipt, TransactionReceiptWithBlockInfo, TransactionStatus,
};
use starknet::providers::{Provider, ProviderError};
use tokio::time::{Instant, Interval};

use dojo_world::migration::TxnConfig;

/// Helper trait to abstract away setting `TxnConfig` configurations before sending a transaction
/// Implemented by types from `starknet-accounts` like `Execution`, `Declaration`, etc...
#[allow(async_fn_in_trait)]
pub trait TransactionExt<T> {
    type R;
    type U;

    /// Sets `fee_estimate_multiplier` and `max_fee_raw` from `TxnConfig` if its present before
    /// calling `send` method on the respective type.
    /// NOTE: If both are specified `max_fee_raw` will take precedence and `fee_estimate_multiplier`
    /// will be ignored by `starknet-rs`
    async fn send_with_cfg(self, txn_config: &TxnConfig) -> Result<Self::R, Self::U>;
}

impl<T> TransactionExt<T> for ExecutionV1<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = InvokeTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        if let TxnConfig { fee_estimate_multiplier: Some(fee_est_mul), .. } = txn_config {
            self = self.fee_estimate_multiplier(*fee_est_mul);
        }

        if let TxnConfig { max_fee_raw: Some(max_fee_r), .. } = txn_config {
            self = self.max_fee(*max_fee_r);
        }

        self.send().await
    }
}

impl<T> TransactionExt<T> for DeclarationV2<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = DeclareTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        if let TxnConfig { fee_estimate_multiplier: Some(fee_est_mul), .. } = txn_config {
            self = self.fee_estimate_multiplier(*fee_est_mul);
        }

        if let TxnConfig { max_fee_raw: Some(max_raw_f), .. } = txn_config {
            self = self.max_fee(*max_raw_f);
        }

        self.send().await
    }
}

impl<T> TransactionExt<T> for AccountDeploymentV1<'_, T>
where
    T: AccountFactory + Sync,
{
    type R = DeployAccountTransactionResult;
    type U = AccountFactoryError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountFactoryError<<T>::SignError>> {
        if let TxnConfig { fee_estimate_multiplier: Some(fee_est_mul), .. } = txn_config {
            self = self.fee_estimate_multiplier(*fee_est_mul);
        }

        if let TxnConfig { max_fee_raw: Some(max_raw_f), .. } = txn_config {
            self = self.max_fee(*max_raw_f);
        }

        self.send().await
    }
}
