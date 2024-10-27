pub mod declarer;
pub mod deployer;
pub mod error;
pub mod invoker;
pub mod waiter;

use std::fmt;

use anyhow::Result;
use colored_json::ToColoredJson;
use starknet::accounts::{
    AccountDeploymentV1, AccountError, AccountFactory, AccountFactoryError, ConnectedAccount,
    DeclarationV2, ExecutionV1,
};
use starknet::core::types::{
    DeclareTransactionResult, DeployAccountTransactionResult, Felt, InvokeTransactionResult,
    TransactionReceiptWithBlockInfo,
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
    pub walnut: bool,
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
        walnut: bool,
    },
    Estimate,
    Simulate,
}

#[derive(Debug)]
pub enum TransactionResult {
    /// In some occasions, the transaction is not sent and it's not an error.
    /// Typically for the deployer/declarer/invoker that have internal logic to check if the
    /// transaction is needed or not.
    Noop,
    /// The transaction hash.
    Hash(Felt),
    /// The transaction hash and it's receipt.
    HashReceipt(Felt, TransactionReceiptWithBlockInfo),
}

impl fmt::Display for TransactionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionResult::Hash(hash) => write!(f, "Transaction hash: {:#066x}", hash),
            TransactionResult::HashReceipt(hash, receipt) => write!(
                f,
                "Transaction hash: {:#066x}\nReceipt: {}",
                hash,
                serde_json::to_string_pretty(&receipt)
                    .expect("Failed to serialize receipt")
                    .to_colored_json_auto()
                    .expect("Failed to colorize receipt")
            ),
            TransactionResult::Noop => write!(f, "Transaction was not sent"),
        }
    }
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

        // TODO: need to fix the wait that is not usable, since we don't have access to the
        // account/provider. Or execution could expose it, or we need it to be stored in the
        // configuration...
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
