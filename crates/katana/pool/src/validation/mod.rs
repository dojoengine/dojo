pub mod error;
pub mod stateful;

use error::InvalidTransactionError;
use katana_primitives::contract::Nonce;
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

    /// tx that is dependent on another tx ie. when the tx nonce is higher than the current account
    /// nonce.
    Dependent {
        tx: T,
        /// The nonce that the tx is using.
        tx_nonce: Nonce,
        /// The current nonce of the sender's account.
        current_nonce: Nonce,
    },
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
