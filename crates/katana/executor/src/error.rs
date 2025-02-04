use katana_primitives::class::ClassHash;
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::Felt;

/// Errors that can be returned by the executor.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {}

/// Errors that can occur during the transaction execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExecutionError {
    #[error("Contract constructor execution error: {reason}")]
    ConstructorExecutionFailed { reason: String },

    #[error("Class with hash {0:#x} is already declared")]
    ClassAlreadyDeclared(ClassHash),

    #[error("Entry point {0:#x} not found in contract")]
    EntryPointNotFound(Felt),

    #[error("Invalid input: {input_descriptor}; {info}")]
    InvalidInput { input_descriptor: String, info: String },

    #[error("Execution failed due to recursion depth exceeded")]
    RecursionDepthExceeded,

    #[error("Contract with address {0} is not deployed")]
    ContractNotDeployed(ContractAddress),

    // The error message is the exact copy of the one defined by blockifier but without using
    // Debug formatting for the struct fields.
    #[error(
        "Invalid transaction nonce of contract at address {address}. Account nonce: \
         {current_nonce:#x}; got: {tx_nonce:#x}."
    )]
    InvalidNonce {
        /// The address of the account contract.
        address: ContractAddress,
        /// The current nonce of the account.
        current_nonce: Nonce,
        /// The nonce of the incoming transaction.
        tx_nonce: Nonce,
    },

    #[error(
        "Insufficient balance: max fee {max_fee} exceeds account balance u256({balance_low}, \
         {balance_high})"
    )]
    InsufficientBalance { max_fee: u128, balance_low: Felt, balance_high: Felt },

    #[error("Actual fee ({actual_fee}) exceeded max fee ({max_fee})")]
    ActualFeeExceedsMaxFee { max_fee: u128, actual_fee: u128 },

    #[error("Transaction max fee ({max_fee:#x}) is too low; min max fee is {min:#x}")]
    MaxFeeTooLow { min: u128, max_fee: u128 },

    #[error("Class with hash {0:#x} is not declared")]
    UndeclaredClass(ClassHash),

    #[error("Fee transfer error: {0}")]
    FeeTransferError(String),

    #[error("Entry point execution error: {reason}")]
    ExecutionFailed { reason: String },

    #[error("Transaction validation error: {reason}")]
    TransactionValidationFailed { reason: String },

    #[error("Transaction reverted: {revert_error}")]
    TransactionReverted { revert_error: String },

    #[error("{0}")]
    Other(String),
}
