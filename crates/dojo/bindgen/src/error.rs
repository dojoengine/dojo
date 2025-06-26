use cainome::parser::Error as CainomeError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Cainome(#[from] CainomeError),
    #[error("Format error: {0}")]
    Format(String),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error("Gathering dojo data failed: {0}")]
    GatherDojoData(String),
}

pub type BindgenResult<T, E = Error> = Result<T, E>;
