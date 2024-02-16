use std::sync::Arc;

use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::sequencer::KatanaSequencer;
use katana_primitives::block::BlockHashOrNumber;
use katana_provider::traits::transaction::TransactionExecutionProvider;
use katana_rpc_api::saya::SayaApiServer;
use katana_rpc_types::error::saya::SayaApiError;
use katana_rpc_types::transaction::{TransactionsExecutionsFilter, TransactionsExecutionsPage};
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
        filter: TransactionsExecutionsFilter,
    ) -> RpcResult<TransactionsExecutionsPage> {
        self.on_io_blocking_task(move |this| {
            let provider = this.sequencer.backend.blockchain.provider();

            let transactions_executions = provider
                .transactions_executions_by_block(BlockHashOrNumber::Num(filter.block_number))
                .map_err(SayaApiError::from)?
                .ok_or(SayaApiError::BlockNotFound)?;

            let total = transactions_executions.len();

            let transactions_executions = transactions_executions
                .into_iter()
                .take(filter.chunk_size as usize)
                .collect::<Vec<_>>();

            let remaining = (total - transactions_executions.len()) as u64;

            Ok(TransactionsExecutionsPage { transactions_executions, remaining })
        })
        .await
    }
}
