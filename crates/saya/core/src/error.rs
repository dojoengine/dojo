#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    DataAvailability(#[from] crate::data_availability::error::Error),
    #[error("Error from Katana client: {0}")]
    KatanaClient(String),
}

pub type SayaResult<T, E = Error> = Result<T, E>;
