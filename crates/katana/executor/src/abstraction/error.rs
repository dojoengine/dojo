use katana_primitives::class::ClassHash;
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::FieldElement;

/// Errors that can be returned by the executor.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {}

/// Errors that can occur during the transaction execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExecutionError {
    #[error("contract constructor execution error: {reason}")]
    ConstructorExecutionFailed { reason: String },

    #[error("class with hash {0:#x} is already declared")]
    ClassAlreadyDeclared(ClassHash),

    #[error("entry point {0:#x} not found in contract")]
    EntryPointNotFound(FieldElement),

    #[error("invalid input: {input_descriptor}; {info}")]
    InvalidInput { input_descriptor: String, info: String },

    #[error("execution failed due to recursion depth exceeded")]
    RecursionDepthExceeded,

    #[error("contract with address {0} is not deployed")]
    ContractNotDeployed(ContractAddress),

    #[error("invalid transaction nonce: expected {expected} got {actual}")]
    InvalidNonce { actual: Nonce, expected: Nonce },

    #[error(
        "insufficient balance: max fee {max_fee} exceeds account balance u256({balance_low}, \
         {balance_high})"
    )]
    InsufficientBalance { max_fee: u128, balance_low: FieldElement, balance_high: FieldElement },

    #[error("actual fee {max_fee} exceeded transaction max fee {actual_fee}")]
    ActualFeeExceedsMaxFee { max_fee: u128, actual_fee: u128 },

    #[error("transaction max fee ({max_fee:#x}) is too low; min max fee is {min:#x}")]
    MaxFeeTooLow { min: u128, max_fee: u128 },

    #[error("class with hash {0:#x} is not declared")]
    UndeclaredClass(ClassHash),

    #[error("fee transfer error: {0}")]
    FeeTransferError(String),

    #[error("entry point execution error: {reason}")]
    ExecutionFailed { reason: String },

    #[error("transaction validation error: {reason}")]
    TransactionValidationFailed { reason: String },

    #[error("transaction reverted: {revert_error}")]
    TransactionReverted { revert_error: String },

    #[error("{0}")]
    Other(String),
}
