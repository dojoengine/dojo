use starknet::core::types::FromStrError;
use starknet::core::utils::CairoShortStringToFeltError;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};

type JsonRpcClientError = <JsonRpcClient<HttpTransport> as Provider>::Error;
type ProviderError = starknet::providers::ProviderError<JsonRpcClientError>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parsing error: {0}")]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
}
