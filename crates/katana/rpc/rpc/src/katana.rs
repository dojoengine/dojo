use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::backend::Backend;
use katana_executor::ExecutorFactory;
use katana_rpc_api::katana::KatanaApiServer;
use katana_rpc_types::account::Account;

#[allow(missing_debug_implementations)]
pub struct KatanaApi<EF: ExecutorFactory> {
    backend: Arc<Backend<EF>>,
}

impl<EF: ExecutorFactory> KatanaApi<EF> {
    pub fn new(backend: Arc<Backend<EF>>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl<EF: ExecutorFactory> KatanaApiServer for KatanaApi<EF> {
    async fn predeployed_accounts(&self) -> Result<Vec<Account>, Error> {
        Ok(self.backend.config.genesis.accounts().map(|e| Account::new(*e.0, e.1)).collect())
    }
}
