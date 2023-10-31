use starknet::core::types::FromStrError;
use starknet::core::utils::CairoShortStringToFeltError;
use starknet::providers::{Provider, ProviderError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parsing error: {0}")]
    Parse(#[from] ParseError),
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

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError<P: Provider> {
    #[error(transparent)]
    Parse(#[from] super::error::ParseError),
    #[error(transparent)]
    Provider(ProviderError<<P as Provider>::Error>),
}
