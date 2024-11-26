use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use katana_primitives::block::{BlockNumber, SealedBlockWithStatus};
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_provider::traits::block::BlockWriter;
use starknet::providers::sequencer::models::{BlockId, StateUpdateWithBlock};
use starknet::providers::{ProviderError, SequencerGatewayProvider};
use tracing::{debug, warn};

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
    pub fn new(
        provider: P,
        feeder_gateway: SequencerGatewayProvider,
        download_batch_size: usize,
    ) -> Self {
        let downloader = Downloader::new(feeder_gateway, download_batch_size);
        Self { provider, downloader }
    }
}

#[async_trait::async_trait]
impl<P: BlockWriter> Stage for Blocks<P> {
    fn id(&self) -> &'static str {
        "Blocks"
    }

    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult {
        // Download all blocks concurrently
        let blocks = self.downloader.download_blocks(input.from, input.to).await?;

        if !blocks.is_empty() {
            debug!(target: "stage", id = %self.id(), total = %blocks.len(), "Storing blocks to storage.");
            // Store blocks to storage
            for block in blocks {
                let StateUpdateWithBlock { state_update, block: fgw_block } = block;

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
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Downloader {
    batch_size: usize,
    client: Arc<SequencerGatewayProvider>,
}

impl Downloader {
    fn new(client: SequencerGatewayProvider, batch_size: usize) -> Self {
        Self { client: Arc::new(client), batch_size }
    }

    /// Fetch blocks in the range [from, to] in batches of `batch_size`.
    async fn download_blocks(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<StateUpdateWithBlock>, Error> {
        debug!(target: "pipeline", %from, %to, "Downloading blocks.");
        let mut blocks = Vec::with_capacity(to.saturating_sub(from) as usize);

        for batch_start in (from..=to).step_by(self.batch_size) {
            let batch_end = (batch_start + self.batch_size as u64 - 1).min(to);
            let batch = self.fetch_blocks_with_retry(batch_start, batch_end).await?;
            blocks.extend(batch);
        }

        Ok(blocks)
    }

    /// Fetch blocks with the given block number with retry mechanism at a batch level.
    async fn fetch_blocks_with_retry(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<StateUpdateWithBlock>, Error> {
        let request = || async move { self.clone().fetch_blocks(from, to).await };

        // Retry only when being rate limited
        let backoff = ExponentialBuilder::default().with_min_delay(Duration::from_secs(9));
        let result = request
            .retry(backoff)
            .notify(|error, _| {
                warn!(target: "pipeline", %from, %to, %error, "Retrying block download.");
            })
            .await?;

        Ok(result)
    }

    async fn fetch_blocks(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<StateUpdateWithBlock>, Error> {
        let total = to.saturating_sub(from) as usize;
        let mut requests = Vec::with_capacity(total);

        for i in from..=to {
            requests.push(self.fetch_block(i));
        }

        let results = futures::future::join_all(requests).await;
        results.into_iter().collect()
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

        let mut stage = Blocks::new(&provider, feeder_gateway, 10);

        let input = StageExecutionInput { from: from_block, to: to_block };
        let _ = stage.execute(&input).await.expect("failed to execute stage");

        // check provider storage
        let block_number = provider.latest_number().expect("failed to get latest block number");
        assert_eq!(block_number, to_block);
    }
}
