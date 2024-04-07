//! Errors related to providers.

/// Possible errors returned by the provider.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    KatanaProvider(#[from] katana_provider::error::ProviderError),
    #[error("Block {0:?} not found.")]
    BlockNotFound(katana_primitives::block::BlockIdOrTag),
    #[error(transparent)]
    StarknetProvider(#[from] starknet::providers::ProviderError),
    #[error(transparent)]
    ValueOutOfRange(#[from] starknet::core::types::ValueOutOfRangeError),
}
