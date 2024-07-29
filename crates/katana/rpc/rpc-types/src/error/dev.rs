use jsonrpsee::core::Error;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;

#[derive(thiserror::Error, Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum DevApiError {
    #[error("Wait for pending transactions.")]
    PendingTransactions,
}

impl From<DevApiError> for Error {
    fn from(err: DevApiError) -> Self {
        Error::Call(CallError::Custom(ErrorObject::owned(err as i32, err.to_string(), None::<()>)))
    }
}
