use dojo_world::contracts::model::ModelError;
use starknet::core::utils::CairoShortStringToFeltError;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};

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
    Model(#[from] ModelError<<JsonRpcClient<HttpTransport> as Provider>::Error>),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    FeltFromStr(#[from] starknet::core::types::FromStrError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
}
