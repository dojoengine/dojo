use jsonrpsee::core::Error;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;
use katana_provider::error::ProviderError;

#[derive(Debug, thiserror::Error, Clone)]
#[repr(i32)]
pub enum SayaApiError {
    #[error("Transaction index out of bounds")]
    TransactionOutOfBounds,
    #[error("Block not found")]
    BlockNotFound,
    #[error("Transaction not found")]
    TransactionNotFound,
    #[error("An unexpected error occured: {reason}")]
    UnexpectedError { reason: String },
}

impl SayaApiError {
    fn code(&self) -> i32 {
        match self {
            SayaApiError::TransactionOutOfBounds => 1,
            SayaApiError::BlockNotFound => 24,
            SayaApiError::TransactionNotFound => 25,
            SayaApiError::UnexpectedError { .. } => 63,
        }
    }
}

impl From<ProviderError> for SayaApiError {
    fn from(value: ProviderError) -> Self {
        SayaApiError::UnexpectedError { reason: value.to_string() }
    }
}

// impl From<SequencerError> for SayaApiError {
//     fn from(value: SequencerError) -> Self {
//         match value {
//             SequencerError::BlockNotFound(_) => SayaApiError::BlockNotFound,
//             err => SayaApiError::UnexpectedError { reason: err.to_string() },
//         }
//     }
// }

impl From<SayaApiError> for Error {
    fn from(err: SayaApiError) -> Self {
        let code = err.code();
        let message = err.to_string();
        let err = ErrorObject::owned(code, message, None::<()>);
        Error::Call(CallError::Custom(err))
    }
}
