use blockifier::execution::errors::{EntryPointExecutionError, PreExecutionError};
use blockifier::state::errors::StateError;
use blockifier::transaction::errors::{
    TransactionExecutionError, TransactionFeeError, TransactionPreValidationError,
};

use crate::implementation::blockifier::utils::to_address;
use crate::ExecutionError;

impl From<TransactionExecutionError> for ExecutionError {
    fn from(error: TransactionExecutionError) -> Self {
        match error {
            TransactionExecutionError::DeclareTransactionError { class_hash } => {
                Self::ClassAlreadyDeclared(class_hash.0)
            }
            TransactionExecutionError::ValidateTransactionError { error, .. } => {
                Self::TransactionValidationFailed { reason: error.to_string() }
            }
            TransactionExecutionError::StateError(e) => Self::from(e),
            TransactionExecutionError::TransactionPreValidationError(e) => Self::from(e),
            TransactionExecutionError::TransactionFeeError(e) => Self::from(e),
            TransactionExecutionError::ExecutionError { error, .. } => Self::from(error),
            TransactionExecutionError::ContractConstructorExecutionFailed(e) => {
                Self::ConstructorExecutionFailed { reason: e.to_string() }
            }
            e => Self::Other(e.to_string()),
        }
    }
}

impl From<EntryPointExecutionError> for ExecutionError {
    fn from(error: EntryPointExecutionError) -> Self {
        match error {
            EntryPointExecutionError::ExecutionFailed { error_trace } => {
                Self::ExecutionFailed { reason: error_trace.to_string() }
            }
            EntryPointExecutionError::InvalidExecutionInput { input_descriptor, info } => {
                Self::InvalidInput { input_descriptor, info }
            }
            EntryPointExecutionError::RecursionDepthExceeded => Self::RecursionDepthExceeded,
            EntryPointExecutionError::StateError(e) => Self::from(e),
            EntryPointExecutionError::PreExecutionError(e) => Self::from(e),
            e => Self::Other(e.to_string()),
        }
    }
}

impl From<PreExecutionError> for ExecutionError {
    fn from(error: PreExecutionError) -> Self {
        match error {
            PreExecutionError::EntryPointNotFound(selector) => Self::EntryPointNotFound(selector.0),
            PreExecutionError::UninitializedStorageAddress(address) => {
                Self::ContractNotDeployed(to_address(address))
            }
            PreExecutionError::StateError(e) => Self::from(e),
            e => Self::Other(e.to_string()),
        }
    }
}

impl From<TransactionPreValidationError> for ExecutionError {
    fn from(error: TransactionPreValidationError) -> Self {
        match error {
            TransactionPreValidationError::InvalidNonce {
                address,
                account_nonce,
                incoming_tx_nonce,
            } => Self::InvalidNonce {
                address: to_address(address),
                tx_nonce: incoming_tx_nonce.0,
                current_nonce: account_nonce.0,
            },
            TransactionPreValidationError::TransactionFeeError(e) => Self::from(e),
            TransactionPreValidationError::StateError(e) => Self::from(e),
        }
    }
}

impl From<TransactionFeeError> for ExecutionError {
    fn from(error: TransactionFeeError) -> Self {
        match error {
            TransactionFeeError::ExecuteFeeTransferError(e) => {
                Self::FeeTransferError(e.to_string())
            }
            TransactionFeeError::MaxFeeTooLow { min_fee, max_fee } => {
                Self::MaxFeeTooLow { min: min_fee.0, max_fee: max_fee.0 }
            }
            TransactionFeeError::StateError(e) => Self::from(e),
            e => Self::Other(e.to_string()),
        }
    }
}

impl From<StateError> for ExecutionError {
    fn from(error: StateError) -> Self {
        match error {
            StateError::UndeclaredClassHash(hash) => Self::UndeclaredClass(hash.0),
            e => Self::Other(e.to_string()),
        }
    }
}
