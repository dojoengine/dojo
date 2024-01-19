use jsonrpsee::core::Error;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;
use katana_core::sequencer_error::SequencerError;
use katana_provider::error::ProviderError;
use starknet::core::types::ContractErrorData;

/// Possible list of errors that can be returned by the Starknet API according to the spec: <https://github.com/starkware-libs/starknet-specs>.
#[derive(Debug, thiserror::Error, Clone)]
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
    #[error("Invalid contract class")]
    InvalidContractClass,
    #[error("Class already declared")]
    ClassAlreadyDeclared,
    #[error("Invalid transaction nonce")]
    InvalidTransactionNonce,
    #[error("Max fee is smaller than the minimal transaction cost (validation plus fee transfer)")]
    InsufficientMaxFee,
    #[error("Account balance is smaller than the transaction's max_fee")]
    InsufficientAccountBalance,
    #[error("Account validation failed")]
    ValidationFailure,
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
    fn code(&self) -> i32 {
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
            StarknetApiError::InvalidContractClass => 50,
            StarknetApiError::ClassAlreadyDeclared => 51,
            StarknetApiError::InvalidTransactionNonce => 52,
            StarknetApiError::InsufficientMaxFee => 53,
            StarknetApiError::InsufficientAccountBalance => 54,
            StarknetApiError::ValidationFailure => 55,
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
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UnexpectedError {
    reason: String,
}

impl From<ProviderError> for StarknetApiError {
    fn from(value: ProviderError) -> Self {
        StarknetApiError::UnexpectedError { reason: value.to_string() }
    }
}

impl From<StarknetApiError> for Error {
    fn from(err: StarknetApiError) -> Self {
        let code = err.code();
        let message = err.to_string();

        let err = match err {
            StarknetApiError::ContractError { revert_error } => {
                ErrorObject::owned(code, message, Some(ContractErrorData { revert_error }))
            }

            StarknetApiError::UnexpectedError { reason } => {
                ErrorObject::owned(code, message, Some(UnexpectedError { reason }))
            }

            _ => ErrorObject::owned(code, message, None::<()>),
        };

        Error::Call(CallError::Custom(err))
    }
}

impl From<SequencerError> for StarknetApiError {
    fn from(value: SequencerError) -> Self {
        match value {
            SequencerError::TransactionExecution(e) => {
                StarknetApiError::ContractError { revert_error: e.to_string() }
            }
            SequencerError::EntryPointExecution(e) => {
                StarknetApiError::ContractError { revert_error: e.to_string() }
            }
            SequencerError::BlockNotFound(_) => StarknetApiError::BlockNotFound,
            SequencerError::ContractNotFound(_) => StarknetApiError::ContractNotFound,
            err => StarknetApiError::UnexpectedError { reason: err.to_string() },
        }
    }
}
