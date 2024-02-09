use std::ops::Range;
use std::sync::Arc;

use futures::StreamExt;
use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::sequencer::KatanaSequencer;
use katana_primitives::block::BlockHashOrNumber;
use katana_provider::traits::block::BlockProvider;
use katana_provider::traits::transaction::TransactionProvider;
use katana_rpc_api::torii::ToriiApiServer;
use katana_rpc_types::error::torii::ToriiApiError;
use katana_rpc_types::transaction::{TransactionsPage, TransactionsPageCursor};
use katana_tasks::TokioTaskSpawner;

#[derive(Clone)]
pub struct ToriiApi {
    sequencer: Arc<KatanaSequencer>,
}

impl ToriiApi {
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
impl ToriiApiServer for ToriiApi {
    async fn get_transactions(
        &self,
        cursor: TransactionsPageCursor,
    ) -> RpcResult<TransactionsPage> {
        self.on_io_blocking_task(move |this| {
            let mut transactions = Vec::new();
            let mut next_cursor = cursor.clone();

            let provider = this.sequencer.backend.blockchain.provider();
            let latest_block_number = this.sequencer.block_number().map_err(ToriiApiError::from)?;

            if cursor.block_number > latest_block_number + 1 {
                return Err(ToriiApiError::BlockNotFound.into());
            }

            if latest_block_number >= cursor.block_number {
                for block_number in cursor.block_number..=latest_block_number {
                    let tx_range = BlockProvider::block_body_indices(
                        provider,
                        BlockHashOrNumber::Num(block_number),
                    )
                    .map_err(ToriiApiError::from)?
                    .ok_or(ToriiApiError::BlockNotFound)?;

                    let mut block_transactions = provider
                        .transaction_in_range(Range::from(tx_range))
                        .map_err(ToriiApiError::from)?
                        .into_iter()
                        .map(|tx| tx.clone().into())
                        .collect::<Vec<_>>();

                    // If the block_number is the cursor block, slice the transactions from the txn
                    // offset
                    if block_number == cursor.block_number {
                        block_transactions = block_transactions
                            .iter()
                            .skip(cursor.transaction_index as usize)
                            .cloned()
                            .collect();
                    }

                    transactions.extend(block_transactions);
                }
            }

            if let Some(pending_state) = this.sequencer.pending_state() {
                let pending_transactions = pending_state
                    .executed_txs
                    .read()
                    .iter()
                    .map(|(tx, _)| tx.clone().into())
                    .collect::<Vec<_>>();

                // If cursor is in the pending block
                if cursor.block_number == latest_block_number + 1 {
                    if cursor.transaction_index as usize > pending_transactions.len() {
                        return Err(ToriiApiError::TransactionOutOfBounds.into());
                    }

                    let mut pending_transactions = pending_transactions
                        .iter()
                        .skip(cursor.transaction_index as usize)
                        .cloned()
                        .collect::<Vec<_>>();

                    // If there are no transactions after the index in the pending block
                    if pending_transactions.is_empty() {
                        // Wait for a new transaction to be added to the pool
                        let mut rx = this.sequencer.pool.add_listener();
                        pending_transactions = match futures::executor::block_on(rx.next()) {
                            // TODO: Consider waiting here for more txns
                            Some(_) => pending_state
                                .executed_txs
                                .read()
                                .iter()
                                .map(|(tx, _)| tx.clone().into())
                                .collect::<Vec<_>>(),
                            None => return Err(ToriiApiError::ChannelDisconnected.into()),
                        };
                    }

                    next_cursor.transaction_index += pending_transactions.len() as u64;
                    transactions.extend(pending_transactions);
                } else {
                    next_cursor.block_number += 1;
                    next_cursor.transaction_index = pending_transactions.len() as u64;
                    transactions.extend(pending_transactions.clone());
                };
            }

            Ok(TransactionsPage { transactions, cursor: next_cursor })
        })
        .await
    }
}
