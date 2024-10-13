pub mod stateful;

use katana_primitives::class::ClassHash;
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::transaction::TxHash;
use katana_primitives::Felt;

use crate::tx::PoolTransaction;

#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct Error {
    /// The hash of the transaction that failed validation.
    pub hash: TxHash,
    /// The actual error object.
    pub error: Box<dyn std::error::Error>,
}

// TODO: figure out how to combine this with ExecutionError
#[derive(Debug, thiserror::Error)]
pub enum InvalidTransactionError {
    /// Error when the account's balance is insufficient to cover the specified transaction fee.
    #[error("Max fee ({max_fee}) exceeds balance ({balance}).")]
    InsufficientFunds {
        /// The specified transaction fee.
        max_fee: u128,
        /// The account's balance of the fee token.
        balance: Felt,
    },

    /// Error when the specified transaction fee is insufficient to cover the minimum fee required
    /// to start the invocation (including the account's validation logic).
    ///
    /// It is a static check that is performed before the transaction is invoked to ensure the
    /// transaction can cover the DA cost, etc.
    ///
    /// This is different from an error due to transaction runs out of gas during execution ie.
    /// the specified max fee is lower than the amount needed to finish the transaction execution
    /// (either validation or execution).
    #[error("Intrinsic transaction fee is too low")]
    IntrinsicFeeTooLow {
        /// The minimum required for the transaction to be executed.
        min: u128,
        /// The specified transaction fee.
        max_fee: u128,
    },

    /// Error when the account's validation logic fails (ie __validate__ function).
    #[error("{error}")]
    ValidationFailure {
        /// The address of the contract that failed validation.
        address: ContractAddress,
        /// The class hash of the account contract.
        class_hash: ClassHash,
        /// The error message returned by Blockifier.
        // TODO: this should be a more specific error type.
        error: String,
    },

    /// Error when the transaction's sender is not an account contract.
    #[error("Sender is not an account")]
    NonAccount {
        /// The address of the contract that is not an account.
        address: ContractAddress,
    },

    /// Error when the transaction is using a nonexpected nonce.
    #[error(
        "Invalid transaction nonce of contract at address {address}. Account nonce: \
         {current_nonce:#x}; got: {tx_nonce:#x}."
    )]
    InvalidNonce {
        /// The address of the contract that has the invalid nonce.
        address: ContractAddress,
        /// The current nonce of the sender's account.
        current_nonce: Nonce,
        /// The nonce that the tx is using.
        tx_nonce: Nonce,
    },
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
