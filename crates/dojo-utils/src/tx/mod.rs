pub mod waiter;

use anyhow::Result;
use starknet::accounts::{
    AccountDeploymentV1, AccountError, AccountFactory, AccountFactoryError, ConnectedAccount,
    DeclarationV2, ExecutionV1,
};
use starknet::core::types::{
    DeclareTransactionResult, DeployAccountTransactionResult, Felt, InvokeTransactionResult,
};

/// The transaction configuration to use when sending a transaction.
#[derive(Debug, Copy, Clone, Default)]
pub struct TxnConfig {
    /// The multiplier for how much the actual transaction max fee should be relative to the
    /// estimated fee. If `None` is provided, the multiplier is set to `1.1`.
    pub fee_estimate_multiplier: Option<f64>,
    pub wait: bool,
    pub receipt: bool,
    pub max_fee_raw: Option<Felt>,
}

#[derive(Debug, Copy, Clone)]
pub enum TxnAction {
    Send {
        wait: bool,
        receipt: bool,
        max_fee_raw: Option<Felt>,
        /// The multiplier for how much the actual transaction max fee should be relative to the
        /// estimated fee. If `None` is provided, the multiplier is set to `1.1`.
        fee_estimate_multiplier: Option<f64>,
    },
    Estimate,
    Simulate,
}

impl TxnConfig {
    pub fn init_wait() -> Self {
        Self { wait: true, ..Default::default() }
    }
}

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
