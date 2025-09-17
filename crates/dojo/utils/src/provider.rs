use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::Provider;
use tracing::trace;

/// Check if the provider is healthy.
///
/// This function will check if the provider is healthy by getting the latest block,
/// and returns an error otherwise.
pub async fn health_check_provider<P: Provider + Sync + std::fmt::Debug + 'static>(
    provider: P,
) -> anyhow::Result<(), anyhow::Error> {
    match provider.get_block_with_tx_hashes(BlockId::Tag(BlockTag::Latest)).await {
        Ok(block) => {
            trace!(
                latest_block = ?block,
                "Provider health check."
            );
            Ok(())
        }
        Err(_) => Err(anyhow::anyhow!("Unhealthy provider. Please check your configuration.")),
    }
}
