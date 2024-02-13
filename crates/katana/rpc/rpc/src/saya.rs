use std::sync::Arc;

use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::sequencer::KatanaSequencer;
use katana_primitives::block::BlockHashOrNumber;
use katana_provider::traits::transaction::TransactionExecutionProvider;
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
            let mut next_cursor = cursor.clone();

            let provider = this.sequencer.backend.blockchain.provider();

            let transactions_executions = provider
                .transactions_executions_by_block(BlockHashOrNumber::Num(cursor.block_number))
                .map_err(SayaApiError::from)?
                .unwrap_or_default();

            // TODO: limit the maximum number of exec info that are sent back to the client.
            // If reach the end -> cursor block is +1, the client can choose to stop.

            Ok(TransactionsExecutionsPage { transactions_executions, cursor: next_cursor })
        })
        .await
    }
}
