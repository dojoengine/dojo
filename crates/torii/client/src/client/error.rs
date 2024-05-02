use dojo_world::contracts::model::ModelError;
use starknet::core::utils::{CairoShortStringToFeltError, ParseCairoShortStringError};
use torii_grpc::types::schema::SchemaError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Subscription service uninitialized")]
    SubscriptionUninitialized,
    #[error("Unknown model: {0}")]
    UnknownModel(String),
    #[error(
        "Invalid amount of values for model {model}. Expected {expected_value_len} values, got \
         {actual_value_len}"
    )]
    InvalidModelValuesLen { model: String, expected_value_len: usize, actual_value_len: usize },
    #[error("Parsing error: {0}")]
    Parse(#[from] ParseError),
    #[error(transparent)]
    GrpcClient(#[from] torii_grpc::client::Error),
    #[error(transparent)]
    RelayClient(#[from] torii_relay::errors::Error),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error("Unsupported query")]
    UnsupportedQuery,
    #[error(transparent)]
    Schema(#[from] SchemaError),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    FeltFromStr(#[from] starknet::core::types::FromStrError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    ParseCairoShortString(#[from] ParseCairoShortStringError),
}
