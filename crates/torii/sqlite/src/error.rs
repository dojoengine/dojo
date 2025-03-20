use std::num::ParseIntError;

use dojo_types::primitive::PrimitiveError;
use dojo_types::schema::EnumError;
use starknet::core::types::FromStrError;
use starknet::core::utils::{CairoShortStringToFeltError, NonAsciiNameError};
use starknet::providers::ProviderError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Parsing error: {0}")]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    QueryError(#[from] QueryError),
    #[error(transparent)]
    PrimitiveError(#[from] PrimitiveError),
    #[error(transparent)]
    EnumError(#[from] EnumError),
    #[error(transparent)]
    ProviderError(#[from] ProviderError),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    NonAsciiName(#[from] NonAsciiNameError),
    #[error(transparent)]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    CairoSerdeError(#[from] cainome::cairo_serde::Error),
    #[error(transparent)]
    FromJsonStr(#[from] serde_json::Error),
    #[error(transparent)]
    FromSlice(#[from] std::array::TryFromSliceError),
    #[error(transparent)]
    FromUtf8(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("Unsupported query")]
    UnsupportedQuery,
    #[error("Missing param: {0}")]
    MissingParam(String),
    #[error("Unsupported value for primitive: {0}")]
    UnsupportedValue(String),
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Exceeds sqlite `JOIN` limit (64)")]
    SqliteJoinLimit,
    #[error("Invalid namespaced model: {0}")]
    InvalidNamespacedModel(String),
    #[error("Invalid timestamp: {0}. Expected valid number of seconds since unix epoch.")]
    InvalidTimestamp(u64),
}
