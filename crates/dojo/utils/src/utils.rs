use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::Provider;
use tracing::trace;

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
        Err(_) => {
            let error_info =
                format!("Unhealthy provider {:?}, please check your configuration.", provider);
            Err(anyhow::anyhow!(error_info))
        }
    }
}
