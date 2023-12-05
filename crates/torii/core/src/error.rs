use std::num::ParseIntError;

use dojo_types::primitive::PrimitiveError;
use dojo_types::schema::EnumError;
use starknet::core::types::{FromByteSliceError, FromStrError};
use starknet::core::utils::CairoShortStringToFeltError;

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
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    FromByteSliceError(#[from] FromByteSliceError),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
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
}
