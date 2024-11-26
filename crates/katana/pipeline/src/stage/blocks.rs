use std::sync::Arc;

use backon::{ExponentialBuilder, Retryable};
use katana_primitives::block::{BlockNumber, SealedBlockWithStatus};
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_provider::traits::block::BlockWriter;
use starknet::providers::sequencer::models::{BlockId, StateUpdateWithBlock};
use starknet::providers::{ProviderError, SequencerGatewayProvider};
use tracing::warn;

use super::{Stage, StageExecutionInput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] ProviderError),
}

#[derive(Debug)]
pub struct Blocks<P> {
    provider: P,
    downloader: Downloader,
}

impl<P> Blocks<P> {
    pub fn new(provider: P, feeder_gateway: SequencerGatewayProvider) -> Self {
        Self { provider, downloader: Downloader::new(feeder_gateway) }
    }
}

#[async_trait::async_trait]
impl<P: BlockWriter> Stage for Blocks<P> {
    fn id(&self) -> &'static str {
        "Blocks"
    }

    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult {
        // Download all blocks concurrently
        let blocks = self.downloader.fetch_blocks_range(input.from, input.to, 10).await?;

        // Then process them sequentially
        for data in blocks {
            let StateUpdateWithBlock { state_update, block: fgw_block } = data;

            let block = SealedBlockWithStatus::from(fgw_block);
            let su = StateUpdates::from(state_update);
            let su = StateUpdatesWithClasses { state_updates: su, ..Default::default() };

            let _ = self.provider.insert_block_with_states_and_receipts(
                block,
                su,
                Vec::new(),
                Vec::new(),
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Downloader {
    client: Arc<SequencerGatewayProvider>,
}

impl Downloader {
    fn new(client: SequencerGatewayProvider) -> Self {
        Self { client: Arc::new(client) }
    }

    /// Fetch blocks in the range [from, to] in batches of `batch_size`.
    async fn fetch_blocks_range(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        batch_size: usize,
    ) -> Result<Vec<StateUpdateWithBlock>, Error> {
        let mut all_results = Vec::with_capacity(to.saturating_sub(from) as usize);

        for batch_start in (from..=to).step_by(batch_size) {
            let batch_end = (batch_start + batch_size as u64 - 1).min(to);

            // fetch in batches and wait on them before proceeding to the next batch
            let mut futures = Vec::new();
            for block_num in batch_start..=batch_end {
                futures.push(self.fetch_block_with_retry(block_num));
            }

            let batch_results = futures::future::join_all(futures).await;
            all_results.extend(batch_results);
        }

        all_results.into_iter().collect()
    }

    /// Fetch a single block with the given block number with retry mechanism.
    async fn fetch_block_with_retry(
        &self,
        block: BlockNumber,
    ) -> Result<StateUpdateWithBlock, Error> {
        let request = || async move {
            #[allow(deprecated)]
            self.clone().fetch_block(block).await
        };

        // Retry only when being rate limited
        let result = request
            .retry(ExponentialBuilder::default())
            .when(|e| matches!(e, Error::Gateway(ProviderError::RateLimited)))
            .notify(|error, _| {
                warn!(target: "pipeline", %block, %error, "Retrying block download.");
            })
            .await?;

        Ok(result)
    }

    /// Fetch a single block with the given block number.
    async fn fetch_block(&self, block: BlockNumber) -> Result<StateUpdateWithBlock, Error> {
        #[allow(deprecated)]
        let res = self.client.get_state_update_with_block(BlockId::Number(block)).await?;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use katana_provider::test_utils::test_provider;
    use katana_provider::traits::block::BlockNumberProvider;
    use starknet::providers::SequencerGatewayProvider;

    use super::Blocks;
    use crate::stage::{Stage, StageExecutionInput};

    #[tokio::test]
    async fn fetch_blocks() {
        let from_block = 308919;
        let to_block = from_block + 2;

        let provider = test_provider();
        let feeder_gateway = SequencerGatewayProvider::starknet_alpha_sepolia();

        let mut stage = Blocks::new(&provider, feeder_gateway);

        let input = StageExecutionInput { from: from_block, to: to_block };
        let _ = stage.execute(&input).await.expect("failed to execute stage");

        // check provider storage
        let block_number = provider.latest_number().expect("failed to get latest block number");
        assert_eq!(block_number, to_block);
    }
}
