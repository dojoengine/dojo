use jsonrpsee::core::Error;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;
use katana_pool::validation::error::InvalidTransactionError;
use katana_pool::PoolError;
use katana_primitives::event::ContinuationTokenError;
use katana_provider::error::ProviderError;
use serde::Serialize;
use serde_json::Value;

/// Possible list of errors that can be returned by the Starknet API according to the spec: <https://github.com/starkware-libs/starknet-specs>.
#[derive(Debug, thiserror::Error, Clone, Serialize)]
#[serde(untagged)]
#[repr(i32)]
pub enum StarknetApiError {
    #[error("Failed to write transaction")]
    FailedToReceiveTxn,
    #[error("Contract not found")]
    ContractNotFound,
    #[error("Invalid message selector")]
    InvalidMessageSelector,
    #[error("Invalid call data")]
    InvalidCallData,
    #[error("Block not found")]
    BlockNotFound,
    #[error("Transaction hash not found")]
    TxnHashNotFound,
    #[error("Invalid transaction index in a block")]
    InvalidTxnIndex,
    #[error("Class hash not found")]
    ClassHashNotFound,
    #[error("Requested page size is too big")]
    PageSizeTooBig,
    #[error("There are no blocks")]
    NoBlocks,
    #[error("The supplied continuation token is invalid or unknown")]
    InvalidContinuationToken,
    #[error("Contract error")]
    ContractError { revert_error: String },
    #[error("Transaction execution error")]
    TransactionExecutionError {
        /// The index of the first transaction failing in a sequence of given transactions.
        transaction_index: usize,
        /// The revert error with the execution trace up to the point of failure.
        execution_error: String,
    },
    #[error("Invalid contract class")]
    InvalidContractClass,
    #[error("Class already declared")]
    ClassAlreadyDeclared,
    // TEMP: adding a reason field temporarily to match what's being returned by the gateway. the
    // gateway includes the information regarding the expected and actual nonce in the error
    // message. but this doesn't break compatibility with the spec.
    #[error("Invalid transaction nonce")]
    InvalidTransactionNonce { reason: String },
    #[error("Max fee is smaller than the minimal transaction cost (validation plus fee transfer)")]
    InsufficientMaxFee,
    #[error("Account balance is smaller than the transaction's max_fee")]
    InsufficientAccountBalance,
    #[error("Account validation failed")]
    ValidationFailure { reason: String },
    #[error("Compilation failed")]
    CompilationFailed,
    #[error("Contract class size is too large")]
    ContractClassSizeIsTooLarge,
    #[error("Sender address in not an account contract")]
    NonAccount,
    #[error("A transaction with the same hash already exists in the mempool")]
    DuplicateTransaction,
    #[error("The compiled class hash did not match the one supplied in the transaction")]
    CompiledClassHashMismatch,
    #[error("The transaction version is not supported")]
    UnsupportedTransactionVersion,
    #[error("The contract class version is not supported")]
    UnsupportedContractClassVersion,
    #[error("An unexpected error occured")]
    UnexpectedError { reason: String },
    #[error("Too many storage keys requested")]
    ProofLimitExceeded,
    #[error("Too many keys provided in a filter")]
    TooManyKeysInFilter,
    #[error("Failed to fetch pending transactions")]
    FailedToFetchPendingTransactions,
}

impl StarknetApiError {
    pub fn code(&self) -> i32 {
        match self {
            StarknetApiError::FailedToReceiveTxn => 1,
            StarknetApiError::ContractNotFound => 20,
            StarknetApiError::InvalidMessageSelector => 21,
            StarknetApiError::InvalidCallData => 22,
            StarknetApiError::BlockNotFound => 24,
            StarknetApiError::InvalidTxnIndex => 27,
            StarknetApiError::ClassHashNotFound => 28,
            StarknetApiError::TxnHashNotFound => 29,
            StarknetApiError::PageSizeTooBig => 31,
            StarknetApiError::NoBlocks => 32,
            StarknetApiError::InvalidContinuationToken => 33,
            StarknetApiError::TooManyKeysInFilter => 34,
            StarknetApiError::FailedToFetchPendingTransactions => 38,
            StarknetApiError::ContractError { .. } => 40,
            StarknetApiError::TransactionExecutionError { .. } => 41,
            StarknetApiError::InvalidContractClass => 50,
            StarknetApiError::ClassAlreadyDeclared => 51,
            StarknetApiError::InvalidTransactionNonce { .. } => 52,
            StarknetApiError::InsufficientMaxFee => 53,
            StarknetApiError::InsufficientAccountBalance => 54,
            StarknetApiError::ValidationFailure { .. } => 55,
            StarknetApiError::CompilationFailed => 56,
            StarknetApiError::ContractClassSizeIsTooLarge => 57,
            StarknetApiError::NonAccount => 58,
            StarknetApiError::DuplicateTransaction => 59,
            StarknetApiError::CompiledClassHashMismatch => 60,
            StarknetApiError::UnsupportedTransactionVersion => 61,
            StarknetApiError::UnsupportedContractClassVersion => 62,
            StarknetApiError::UnexpectedError { .. } => 63,
            StarknetApiError::ProofLimitExceeded => 10000,
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }

    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            StarknetApiError::ContractError { .. }
            | StarknetApiError::UnexpectedError { .. }
            | StarknetApiError::TransactionExecutionError { .. } => Some(serde_json::json!(self)),

            StarknetApiError::InvalidTransactionNonce { reason }
            | StarknetApiError::ValidationFailure { reason } => {
                Some(Value::String(reason.to_string()))
            }

            _ => None,
        }
    }
}

impl From<StarknetApiError> for Error {
    fn from(err: StarknetApiError) -> Self {
        Error::Call(CallError::Custom(ErrorObject::owned(err.code(), err.message(), err.data())))
    }
}
impl From<ProviderError> for StarknetApiError {
    fn from(value: ProviderError) -> Self {
        StarknetApiError::UnexpectedError { reason: value.to_string() }
    }
}

impl From<ContinuationTokenError> for StarknetApiError {
    fn from(value: ContinuationTokenError) -> Self {
        match value {
            ContinuationTokenError::InvalidToken => StarknetApiError::InvalidContinuationToken,
            ContinuationTokenError::ParseFailed(e) => {
                StarknetApiError::UnexpectedError { reason: e.to_string() }
            }
        }
    }
}

impl From<anyhow::Error> for StarknetApiError {
    fn from(value: anyhow::Error) -> Self {
        StarknetApiError::UnexpectedError { reason: value.to_string() }
    }
}

impl From<PoolError> for StarknetApiError {
    fn from(error: PoolError) -> Self {
        match error {
            PoolError::InvalidTransaction(err) => err.into(),
            PoolError::Internal(err) => {
                StarknetApiError::UnexpectedError { reason: err.to_string() }
            }
        }
    }
}

impl From<Box<InvalidTransactionError>> for StarknetApiError {
    fn from(error: Box<InvalidTransactionError>) -> Self {
        match error.as_ref() {
            InvalidTransactionError::InsufficientFunds { .. } => Self::InsufficientAccountBalance,
            InvalidTransactionError::IntrinsicFeeTooLow { .. } => Self::InsufficientMaxFee,
            InvalidTransactionError::NonAccount { .. } => Self::NonAccount,
            InvalidTransactionError::InvalidNonce { .. } => {
                Self::InvalidTransactionNonce { reason: error.to_string() }
            }
            InvalidTransactionError::ValidationFailure { error, .. } => {
                Self::ValidationFailure { reason: error.to_string() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    #[rustfmt::skip]
    #[rstest]
    #[case(StarknetApiError::NoBlocks, 32, "There are no blocks")]
    #[case(StarknetApiError::BlockNotFound, 24, "Block not found")]
    #[case(StarknetApiError::InvalidCallData, 22, "Invalid call data")]
    #[case(StarknetApiError::ContractNotFound, 20, "Contract not found")]
    #[case(StarknetApiError::CompilationFailed, 56, "Compilation failed")]
    #[case(StarknetApiError::ClassHashNotFound, 28, "Class hash not found")]
    #[case(StarknetApiError::TxnHashNotFound, 29, "Transaction hash not found")]
    #[case(StarknetApiError::ClassAlreadyDeclared, 51, "Class already declared")]
    #[case(StarknetApiError::InvalidContractClass, 50, "Invalid contract class")]
    #[case(StarknetApiError::PageSizeTooBig, 31, "Requested page size is too big")]
    #[case(StarknetApiError::FailedToReceiveTxn, 1, "Failed to write transaction")]
    #[case(StarknetApiError::InvalidMessageSelector, 21, "Invalid message selector")]
    #[case(StarknetApiError::NonAccount, 58, "Sender address in not an account contract")]
    #[case(StarknetApiError::InvalidTxnIndex, 27, "Invalid transaction index in a block")]
    #[case(StarknetApiError::ProofLimitExceeded, 10000, "Too many storage keys requested")]
    #[case(StarknetApiError::TooManyKeysInFilter, 34, "Too many keys provided in a filter")]
    #[case(StarknetApiError::ContractClassSizeIsTooLarge, 57, "Contract class size is too large")]
    #[case(StarknetApiError::FailedToFetchPendingTransactions, 38, "Failed to fetch pending transactions")]
    #[case(StarknetApiError::UnsupportedTransactionVersion, 61, "The transaction version is not supported")]
    #[case(StarknetApiError::UnsupportedContractClassVersion, 62, "The contract class version is not supported")]
    #[case(StarknetApiError::InvalidContinuationToken, 33, "The supplied continuation token is invalid or unknown")]
    #[case(StarknetApiError::DuplicateTransaction, 59, "A transaction with the same hash already exists in the mempool")]
    #[case(StarknetApiError::InsufficientAccountBalance, 54, "Account balance is smaller than the transaction's max_fee")]
    #[case(StarknetApiError::CompiledClassHashMismatch, 60, "The compiled class hash did not match the one supplied in the transaction")]
    #[case(StarknetApiError::InsufficientMaxFee, 53, "Max fee is smaller than the minimal transaction cost (validation plus fee transfer)")]
    fn test_starknet_api_error_to_error_conversion_data_none(
        #[case] starknet_error: StarknetApiError,
        #[case] expected_code: i32,
        #[case] expected_message: &str,
    ) {
        let error: Error = starknet_error.into();
        match error {
            Error::Call(CallError::Custom(err)) => {
                assert_eq!(err.code(), expected_code);
                assert_eq!(err.message(), expected_message);
                assert!(err.data().is_none(), "data should be None");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[rstest]
    #[case(
        StarknetApiError::ContractError {
            revert_error: "Contract error message".to_string(),
        },
        40,
        "Contract error",
        json!({
            "revert_error": "Contract error message".to_string()
        }),
    )]
    #[case(
        StarknetApiError::TransactionExecutionError {
            transaction_index: 1,
            execution_error: "Transaction execution error message".to_string(),
        },
        41,
        "Transaction execution error",
        json!({
            "transaction_index": 1,
            "execution_error": "Transaction execution error message".to_string()
        }),
    )]
    #[case(
        StarknetApiError::UnexpectedError {
            reason: "Unexpected error reason".to_string(),
        },
        63,
        "An unexpected error occured",
        json!({
            "reason": "Unexpected error reason".to_string()
        }),
    )]
    #[case(
    	StarknetApiError::InvalidTransactionNonce {
     		reason: "Wrong nonce".to_string()
      	},
     	52,
      	"Invalid transaction nonce",
       	Value::String("Wrong nonce".to_string())
    )]
    #[case(
    	StarknetApiError::ValidationFailure {
     		reason: "Invalid signature".to_string()
      	},
     	55,
      	"Account validation failed",
       	Value::String("Invalid signature".to_string())
    )]
    fn test_starknet_api_error_to_error_conversion_data_some(
        #[case] starknet_error: StarknetApiError,
        #[case] expected_code: i32,
        #[case] expected_message: &str,
        #[case] expected_data: serde_json::Value,
    ) {
        let error: Error = starknet_error.into();
        match error {
            Error::Call(CallError::Custom(err)) => {
                assert_eq!(err.code(), expected_code);
                assert_eq!(err.message(), expected_message);
                assert_eq!(
                    err.data().unwrap().to_string(),
                    expected_data.to_string(),
                    "data should exist"
                );
            }
            _ => panic!("Unexpected error variant"),
        }
    }
}
