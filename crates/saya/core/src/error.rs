use std::env;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    DataAvailability(#[from] crate::data_availability::error::Error),
    #[error("Error from Katana client: {0}")]
    KatanaClient(String),
    #[error(transparent)]
    KatanaProvider(#[from] katana_provider::error::ProviderError),
    #[error(transparent)]
    SayaProvider(#[from] saya_provider::error::ProviderError),
    #[error("Block {0:?} not found.")]
    BlockNotFound(katana_primitives::block::BlockIdOrTag),
    // #[error(transparent)]
    // Snos(#[from] snos::error::SnOsError),
    #[error("Invalid chain_id ")]
    InvalidChainId,
    #[error(transparent)]
    ProverError(#[from] ProverError),
    #[error("{0}")]
    TimeoutError(String),
    #[error("{0}")]
    TransactionRejected(String),
    #[error("{0}")]
    TransactionFailed(String),
    #[error("{0}")]
    SerdeFeltError(#[from] serde_felt::Error),
}

pub type SayaResult<T, E = Error> = Result<T, E>;

#[derive(thiserror::Error, Debug)]
pub enum ProverError {
    #[error(transparent)]
    ProverSdkError(#[from] prover_sdk::errors::SdkErrors),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    EnvVarError(#[from] env::VarError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    RequestError(#[from] reqwest::Error),
    #[error("Failed to convert calls to felts: {0}")]
    SerdeFeltError(String),
    #[error(transparent)]
    SharpError(#[from] herodotus_sharp_playground::SharpSdkError),
    #[error(transparent)]
    Cairo1PlaygroundError(#[from] cairo1_playground::error::Error),
    #[error("Failed to send transaction: {0}")]
    SendTransactionError(String),
    #[error("Failed to prove: {0}")]
    ProvingFailed(String),
}
