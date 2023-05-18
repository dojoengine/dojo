use jsonrpsee::core::Error;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;

#[derive(thiserror::Error, Clone, Copy, Debug)]
pub enum KatanaApiError {}

impl From<KatanaApiError> for Error {
    fn from(err: KatanaApiError) -> Self {
        Error::Call(CallError::Custom(ErrorObject::owned(err as i32, err.to_string(), None::<()>)))
    }
}

#[rpc(server, client, namespace = "katana")]
pub trait KatanaApi {
    #[method(name = "generateBlock")]
    async fn generate_block(&self) -> Result<(), Error>;
}
