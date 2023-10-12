use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Not a list: {0}")]
    NotList(String),
    #[error("Not a string: {0}")]
    NotString(String),
    #[error("Not a number: {0}")]
    NotNumber(String),
}
