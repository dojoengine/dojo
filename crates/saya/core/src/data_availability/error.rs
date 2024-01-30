#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Data availability error occurred: {0}")]
    Generic(String),
}

pub type DataAvailabilityResult<T, E = Error> = Result<T, E>;
