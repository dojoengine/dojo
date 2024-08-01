use std::num::ParseIntError;

use dojo_types::primitive::PrimitiveError;
use dojo_types::schema::EnumError;
use starknet::core::types::FromStrError;
use starknet::core::utils::{CairoShortStringToFeltError, NonAsciiNameError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parsing error: {0}")]
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
    SchemaError(#[from] SchemaError),
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("Missing expected data")]
    MissingExpectedData,
    #[error("Unsupported type")]
    UnsupportedType,
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
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("unsupported query")]
    UnsupportedQuery,
    #[error("missing param: {0}")]
    MissingParam(String),
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("exceeds sqlite `JOIN` limit (64)")]
    SqliteJoinLimit,
    #[error("invalid namespaced model: {0}")]
    InvalidNamespacedModel(String),
}
