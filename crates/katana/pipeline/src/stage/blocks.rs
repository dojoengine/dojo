use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use katana_feeder_gateway::client::SequencerGateway;
use katana_feeder_gateway::types::StateUpdateWithBlock;
use katana_primitives::block::{BlockIdOrTag, BlockNumber, SealedBlockWithStatus};
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_primitives::transaction::TxWithHash;
use katana_provider::traits::block::BlockWriter;
use tracing::{debug, warn};

use super::{Stage, StageExecutionInput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] katana_feeder_gateway::client::Error),
}

#[derive(Debug)]
pub struct Blocks<P> {
    provider: P,
    downloader: Downloader,
}

impl<P> Blocks<P> {
    pub fn new(provider: P, feeder_gateway: SequencerGateway, download_batch_size: usize) -> Self {
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
                let StateUpdateWithBlock { state_update, block } = block;

                let transactions: Vec<TxWithHash> = block
                    .transactions
                    .into_iter()
                    .map(|tx| tx.try_into())
                    .collect::<Result<Vec<_>, _>>()?;

                let block: SealedBlockWithStatus = block.into;

                // let su = StateUpdates::from(state_update);
                // let su = StateUpdatesWithClasses { state_updates: su, ..Default::default() };

                // let _ = self.provider.insert_block_with_states_and_receipts(
                //     block,
                //     su,
                //     Vec::new(),
                //     Vec::new(),
                // );
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Downloader {
    batch_size: usize,
    client: Arc<SequencerGateway>,
}

impl Downloader {
    fn new(client: SequencerGateway, batch_size: usize) -> Self {
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
        Ok(self.client.get_state_update_with_block(BlockIdOrTag::Number(block)).await?)
    }
}

#[cfg(test)]
mod tests {
    use katana_feeder_gateway::client::SequencerGateway;
    use katana_provider::test_utils::test_provider;
    use katana_provider::traits::block::BlockNumberProvider;

    use super::Blocks;
    use crate::stage::{Stage, StageExecutionInput};

    #[tokio::test]
    async fn fetch_blocks() {
        let from_block = 308919;
        let to_block = from_block + 2;

        let provider = test_provider();
        let feeder_gateway = SequencerGateway::sn_sepolia();

        let mut stage = Blocks::new(&provider, feeder_gateway, 10);

        let input = StageExecutionInput { from: from_block, to: to_block };
        let _ = stage.execute(&input).await.expect("failed to execute stage");

        // check provider storage
        let block_number = provider.latest_number().expect("failed to get latest block number");
        assert_eq!(block_number, to_block);
    }
}
