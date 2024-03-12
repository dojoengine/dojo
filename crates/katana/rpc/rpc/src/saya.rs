use std::sync::Arc;

use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::sequencer::KatanaSequencer;
use katana_executor::ExecutorFactory;
use katana_primitives::block::BlockHashOrNumber;
use katana_provider::traits::transaction::TransactionTraceProvider;
use katana_rpc_api::saya::SayaApiServer;
use katana_rpc_types::error::saya::SayaApiError;
use katana_rpc_types::transaction::{TransactionsExecutionsPage, TransactionsPageCursor};
use katana_tasks::TokioTaskSpawner;

pub struct SayaApi<EF: ExecutorFactory> {
    sequencer: Arc<KatanaSequencer<EF>>,
}

impl<EF: ExecutorFactory> Clone for SayaApi<EF> {
    fn clone(&self) -> Self {
        Self { sequencer: self.sequencer.clone() }
    }
}

impl<EF: ExecutorFactory> SayaApi<EF> {
    pub fn new(sequencer: Arc<KatanaSequencer<EF>>) -> Self {
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
impl<EF: ExecutorFactory> SayaApiServer for SayaApi<EF> {
    async fn get_transactions_executions(
        &self,
        cursor: TransactionsPageCursor,
    ) -> RpcResult<TransactionsExecutionsPage> {
        self.on_io_blocking_task(move |this| {
            let provider = this.sequencer.backend.blockchain.provider();
            let mut next_cursor = cursor;

            let transactions_executions = provider
                .transactions_executions_by_block(BlockHashOrNumber::Num(cursor.block_number))
                .map_err(SayaApiError::from)?
                .ok_or(SayaApiError::BlockNotFound)?;

            let total_execs = transactions_executions.len() as u64;

            let transactions_executions = transactions_executions
                .into_iter()
                .skip(cursor.transaction_index as usize)
                .take(cursor.chunk_size as usize)
                .collect::<Vec<_>>();

            if cursor.transaction_index + cursor.chunk_size >= total_execs {
                // All transactions of the block pointed by the cursor were fetched.
                // Indicate to the client this situation by setting the block number
                // to the next block and transaction index to 0.
                next_cursor.block_number = cursor.block_number + 1;
                next_cursor.transaction_index = 0;
            } else {
                next_cursor.transaction_index +=
                    cursor.transaction_index + transactions_executions.len() as u64;
            }

            Ok(TransactionsExecutionsPage { transactions_executions, cursor: next_cursor })
        })
        .await
    }
}
