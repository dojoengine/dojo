use std::ops::Range;
use std::sync::Arc;

use futures::StreamExt;
use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::sequencer::KatanaSequencer;
use katana_primitives::block::BlockHashOrNumber;
use katana_provider::traits::block::BlockProvider;
use katana_provider::traits::transaction::TransactionProvider;
use katana_rpc_api::saya::SayaApiServer;
use katana_rpc_types::error::saya::SayaApiError;
use katana_rpc_types::transaction::{TransactionsExecutionsPage, TransactionsPageCursor};
use katana_tasks::TokioTaskSpawner;

#[derive(Clone)]
pub struct SayaApi {
    sequencer: Arc<KatanaSequencer>,
}

impl SayaApi {
    pub fn new(sequencer: Arc<KatanaSequencer>) -> Self {
        Self { sequencer }
    }

    async fn on_io_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        TokioTaskSpawner::new().unwrap().spawn_blocking(move || func(this)).await.unwrap()
    }
}

#[async_trait]
impl SayaApiServer for SayaApi {
    async fn get_transactions_executions(
        &self,
        cursor: TransactionsPageCursor,
    ) -> RpcResult<TransactionsExecutionsPage> {
        self.on_io_blocking_task(move |this| {
            let mut transactions_executions = Vec::new();
            let mut next_cursor = cursor.clone();

            let provider = this.sequencer.backend.blockchain.provider();
            let latest_block_number = this.sequencer.block_number().map_err(SayaApiError::from)?;

            Ok(TransactionsExecutionsPage { transactions_executions, cursor: next_cursor })
        })
        .await
    }
}
