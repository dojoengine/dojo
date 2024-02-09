use jsonrpsee::core::Error;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;
use katana_core::sequencer_error::SequencerError;
use katana_provider::error::ProviderError;

#[derive(Debug, thiserror::Error, Clone)]
#[repr(i32)]
pub enum ToriiApiError {
    #[error("Transaction index out of bounds")]
    TransactionOutOfBounds,
    #[error("Block not found")]
    BlockNotFound,
    #[error("Transaction not found")]
    TransactionNotFound,
    #[error("Long poll expired")]
    ChannelDisconnected,
    #[error("An unexpected error occured")]
    UnexpectedError { reason: String },
}

impl ToriiApiError {
    fn code(&self) -> i32 {
        match self {
            ToriiApiError::TransactionOutOfBounds => 1,
            ToriiApiError::BlockNotFound => 24,
            ToriiApiError::TransactionNotFound => 25,
            ToriiApiError::ChannelDisconnected => 42,
            ToriiApiError::UnexpectedError { .. } => 63,
        }
    }
}

impl From<ProviderError> for ToriiApiError {
    fn from(value: ProviderError) -> Self {
        ToriiApiError::UnexpectedError { reason: value.to_string() }
    }
}

impl From<SequencerError> for ToriiApiError {
    fn from(value: SequencerError) -> Self {
        match value {
            SequencerError::BlockNotFound(_) => ToriiApiError::BlockNotFound,
            err => ToriiApiError::UnexpectedError { reason: err.to_string() },
        }
    }
}

impl From<ToriiApiError> for Error {
    fn from(err: ToriiApiError) -> Self {
        let code = err.code();
        let message = err.to_string();
        let err = ErrorObject::owned(code, message, None::<()>);
        Error::Call(CallError::Custom(err))
    }
}
