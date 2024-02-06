use jsonrpsee::core::Error;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;

#[derive(thiserror::Error, Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum KatanaApiError {
    #[error("Failed to change next block timestamp.")]
    FailedToChangeNextBlockTimestamp = 1,
    #[error("Failed to dump state.")]
    FailedToDumpState = 2,
    #[error("Failed to update storage.")]
    FailedToUpdateStorage = 3,
}

impl From<KatanaApiError> for Error {
    fn from(err: KatanaApiError) -> Self {
        Error::Call(CallError::Custom(ErrorObject::owned(err as i32, err.to_string(), None::<()>)))
    }
}
