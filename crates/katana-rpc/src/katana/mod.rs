use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::sequencer::Sequencer;

use self::api::{KatanaApiError, KatanaApiServer};

pub mod api;

pub struct KatanaRpc<S> {
    sequencer: Arc<S>,
}

impl<S: Sequencer + Send + Sync + 'static> KatanaRpc<S> {
    pub fn new(sequencer: Arc<S>) -> Self {
        Self { sequencer }
    }
}

#[async_trait]
impl<S: Sequencer + Send + Sync + 'static> KatanaApiServer for KatanaRpc<S> {
    async fn generate_block(&self) -> Result<(), Error> {
        self.sequencer.generate_new_block().await;
        Ok(())
    }

    async fn next_block_timestamp(&self) -> Result<u64, Error> {
        Ok(self.sequencer.next_block_timestamp().await.0)
    }

    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .set_next_block_timestamp(timestamp)
            .await
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .increase_next_block_timestamp(timestamp)
            .await
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }
}
