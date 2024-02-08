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
}

pub type SayaResult<T, E = Error> = Result<T, E>;
