use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::sequencer::Sequencer;
use tokio::sync::RwLock;

use self::api::KatanaApiServer;

pub mod api;

pub struct KatanaRpc<S> {
    sequencer: Arc<RwLock<S>>,
}

impl<S: Sequencer + Send + Sync + 'static> KatanaRpc<S> {
    pub fn new(sequencer: Arc<RwLock<S>>) -> Self {
        Self { sequencer }
    }
}

#[async_trait]
impl<S: Sequencer + Send + Sync + 'static> KatanaApiServer for KatanaRpc<S> {
    async fn generate_block(&self) -> Result<(), Error> {
        self.sequencer.write().await.generate_new_block();
        Ok(())
    }
}
