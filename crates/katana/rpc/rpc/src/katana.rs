use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::sequencer::KatanaSequencer;
use katana_rpc_api::katana::KatanaApiServer;
use katana_rpc_types::account::Account;

pub struct KatanaApi {
    sequencer: Arc<KatanaSequencer>,
}

impl KatanaApi {
    pub fn new(sequencer: Arc<KatanaSequencer>) -> Self {
        Self { sequencer }
    }
}

#[async_trait]
impl KatanaApiServer for KatanaApi {
    async fn predeployed_accounts(&self) -> Result<Vec<Account>, Error> {
        Ok(self
            .sequencer
            .backend()
            .config
            .genesis
            .accounts()
            .map(|e| Account::new(*e.0, e.1))
            .collect())
    }
}
