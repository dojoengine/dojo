use futures::channel::mpsc::Receiver;
use jsonrpsee::core::Error;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;
use katana_core::service::block_producer::TxWithOutcome;
use katana_provider::error::ProviderError;

use crate::transaction::TransactionsPageCursor;

#[derive(Debug, thiserror::Error)]
#[repr(i32)]
pub enum ToriiApiError {
    #[error("Block not found")]
    BlockNotFound,
    #[error("Transaction index out of bounds")]
    TransactionOutOfBounds,
    #[error("Transaction not found")]
    TransactionNotFound,
    #[error("Transaction receipt not found")]
    TransactionReceiptNotFound,
    #[error("Transactions not ready")]
    TransactionsNotReady { rx: Receiver<Vec<TxWithOutcome>>, cursor: TransactionsPageCursor },
    #[error("Long poll expired")]
    ChannelDisconnected,
    #[error("An unexpected error occured: {reason}")]
    UnexpectedError { reason: String },
}

impl ToriiApiError {
    fn code(&self) -> i32 {
        match self {
            ToriiApiError::BlockNotFound => 24,
            ToriiApiError::TransactionOutOfBounds => 34,
            ToriiApiError::TransactionNotFound => 35,
            ToriiApiError::TransactionReceiptNotFound => 36,
            ToriiApiError::TransactionsNotReady { .. } => 37,
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

impl From<ToriiApiError> for Error {
    fn from(err: ToriiApiError) -> Self {
        let code = err.code();
        let message = err.to_string();
        let err = ErrorObject::owned(code, message, None::<()>);
        Error::Call(CallError::Custom(err))
    }
}
