use starknet::providers::ProviderError;
use torii_sqlite::error::ParseError;

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Provider(ProviderError),
}
