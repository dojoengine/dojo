pub mod stateful;

use katana_executor::ExecutionError;
use katana_primitives::transaction::TxHash;

use crate::tx::PoolTransaction;

#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct Error {
    /// The hash of the transaction that failed validation.
    pub hash: TxHash,
    /// The actual error object.
    pub error: Box<dyn std::error::Error>,
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidTransactionError {
    #[error("account has insufficient funds to cover the tx fee")]
    InsufficientFunds {},

    #[error("the specified tx max fee is insufficient")]
    InsufficientMaxFee {},

    #[error("invalid signature")]
    InvalidSignature { error: ExecutionError },

    #[error("sender is not an account")]
    NonAccount,

    #[error("nonce mismatch")]
    InvalidNonce,

    #[error("max l1 gas amount too low")]
    MaxL1GasAmountTooLow,

    #[error("placeholder")]
    MaxL1GasPriceTooLow,
}

pub type ValidationResult<T> = Result<ValidationOutcome<T>, Error>;

/// A trait for validating transactions before they are added to the transaction pool.
pub trait Validator {
    type Transaction: PoolTransaction;

    /// Validate a transaction.
    fn validate(&self, tx: Self::Transaction) -> ValidationResult<Self::Transaction>;

    /// Validate a batch of transactions.
    fn validate_all(
        &self,
        txs: Vec<Self::Transaction>,
    ) -> Vec<ValidationResult<Self::Transaction>> {
        txs.into_iter().map(|tx| self.validate(tx)).collect()
    }
}

// outcome of the validation phase. the variant of this enum determines on which pool
// the tx should be inserted into.
#[derive(Debug)]
pub enum ValidationOutcome<T> {
    /// tx that is or may eventually be valid after some nonce changes.
    Valid(T),
    /// tx that will never be valid, eg. due to invalid signature, nonce lower than current, etc.
    Invalid { tx: T, error: InvalidTransactionError },
}

/// A no-op validator that does nothing and assume all incoming transactions are valid.
#[derive(Debug)]
pub struct NoopValidator<T>(std::marker::PhantomData<T>);

impl<T> NoopValidator<T> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: PoolTransaction> Validator for NoopValidator<T> {
    type Transaction = T;

    fn validate(&self, tx: Self::Transaction) -> ValidationResult<Self::Transaction> {
        ValidationResult::Ok(ValidationOutcome::Valid(tx))
    }
}

impl<T> Default for NoopValidator<T> {
    fn default() -> Self {
        Self::new()
    }
}
