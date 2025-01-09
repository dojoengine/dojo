use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid type: {0}")]
    InvalidType(String),

    #[error("Type not found: {0}")]
    TypeNotFound(String),

    #[error("Invalid enum: {0}")]
    InvalidEnum(String),

    #[error("Invalid field: {0}")]
    InvalidField(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Invalid domain: {0}")]
    InvalidDomain(String),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Field not found: {0}")]
    FieldNotFound(String),
}
