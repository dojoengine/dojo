use blockifier::execution::errors::{EntryPointExecutionError, PreExecutionError};
use blockifier::execution::execution_utils::format_panic_data;
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
                Self::ClassAlreadyDeclared(class_hash.0.into())
            }
            TransactionExecutionError::ValidateTransactionError(e) => {
                Self::TransactionValidationFailed(Box::new(Self::from(e)))
            }
            TransactionExecutionError::StateError(e) => Self::from(e),
            TransactionExecutionError::TransactionPreValidationError(e) => Self::from(e),
            TransactionExecutionError::TransactionFeeError(e) => Self::from(e),
            TransactionExecutionError::ExecutionError(e) => Self::from(e),
            TransactionExecutionError::ContractConstructorExecutionFailed(e) => {
                Self::ConstructorExecutionFailed(Box::new(Self::from(e)))
            }
            e => Self::Other(e.to_string()),
        }
    }
}

impl From<EntryPointExecutionError> for ExecutionError {
    fn from(error: EntryPointExecutionError) -> Self {
        match error {
            EntryPointExecutionError::ExecutionFailed { error_data } => {
                Self::ExecutionFailed { reason: format_panic_data(&error_data) }
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
            PreExecutionError::EntryPointNotFound(selector) => {
                Self::EntryPointNotFound(selector.0.into())
            }
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
                account_nonce,
                incoming_tx_nonce,
                ..
            } => Self::InvalidNonce {
                actual: account_nonce.0.into(),
                expected: incoming_tx_nonce.0.into(),
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
            StateError::UndeclaredClassHash(hash) => Self::UndeclaredClass(hash.0.into()),
            e => Self::Other(e.to_string()),
        }
    }
}
