use std::future::IntoFuture;
use std::time::Duration;

use anyhow::Result;
use futures::future::BoxFuture;
use katana_feeder_gateway::client::SequencerGateway;
use katana_pipeline::PipelineHandle;
use katana_primitives::block::{BlockIdOrTag, BlockTag};
use tracing::{error, info, trace};

type TipWatcherFut = BoxFuture<'static, Result<()>>;

#[derive(Debug)]
pub struct ChainTipWatcher {
    /// The feeder gateway client for fetching the latest block.
    client: SequencerGateway,
    /// The pipeline handle for setting the tip.
    pipeline_handle: PipelineHandle,
    /// Interval for checking the new tip.
    watch_interval: Duration,
}

impl ChainTipWatcher {
    pub fn new(client: SequencerGateway, pipeline_handle: PipelineHandle) -> Self {
        let watch_interval = Duration::from_secs(30);
        Self { client, pipeline_handle, watch_interval }
    }

    pub async fn run(&self) -> Result<()> {
        let interval_in_secs = self.watch_interval.as_secs();
        info!(target: "node", interval = %interval_in_secs, "Chain tip watcher started.");

        let mut prev_tip = 0;

        loop {
            let block = self.client.get_block(BlockIdOrTag::Tag(BlockTag::Latest)).await?;
            let block_number = block.block_number.expect("must exist for latest block");

            if prev_tip != block_number {
                trace!(target: "node", block = %block_number, "New tip received");
                self.pipeline_handle.set_tip(block_number);
                prev_tip = block_number;
            }

            tokio::time::sleep(self.watch_interval).await;
        }
    }
}

impl IntoFuture for ChainTipWatcher {
    type Output = Result<()>;
    type IntoFuture = TipWatcherFut;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.run().await.inspect_err(|error| {
                error!(target: "pipeline", %error, "Tip watcher failed.");
            })
        })
    }
}
