use starknet::core::types::FromStrError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error originated from the gRPC client.
    #[error(transparent)]
    GrpcClient(torii_grpc::client::Error),
    #[error(transparent)]
    Parsing(FromStrError),
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<torii_grpc::client::Error> for Error {
    fn from(value: torii_grpc::client::Error) -> Self {
        Self::GrpcClient(value)
    }
}

impl From<FromStrError> for Error {
    fn from(value: FromStrError) -> Self {
        Self::Parsing(value)
    }
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value)
    }
}
