use starknet::providers::{Provider, ProviderError};
use torii_core::error::ParseError;

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError<P: Provider> {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Provider(ProviderError<<P as Provider>::Error>),
}
