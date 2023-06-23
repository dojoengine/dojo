use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::accounts::Account;
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
        let mut starknet = self.sequencer.mut_starknet().await;
        starknet.generate_latest_block();
        starknet.generate_pending_block();
        Ok(())
    }

    async fn next_block_timestamp(&self) -> Result<u64, Error> {
        Ok(self.sequencer.starknet().await.block_context.block_timestamp.0)
    }

    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .mut_starknet()
            .await
            .set_next_block_timestamp(timestamp)
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .mut_starknet()
            .await
            .increase_next_block_timestamp(timestamp)
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }

    async fn predeployed_accounts(&self) -> Result<Vec<Account>, Error> {
        Ok(self.sequencer.starknet().await.predeployed_accounts.accounts.clone())
    }
}
