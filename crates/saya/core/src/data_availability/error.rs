#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Data availability error occurred: {0}")]
    Generic(String),
    #[error("Data availability client error: {0}")]
    Client(String),
    #[error("Invalid data availability chain: {0}")]
    InvalidChain(String),
}

pub type DataAvailabilityResult<T, E = Error> = Result<T, E>;
