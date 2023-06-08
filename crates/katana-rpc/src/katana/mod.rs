use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::sequencer::Sequencer;

use self::api::KatanaApiServer;

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
}
