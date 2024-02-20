use std::sync::Arc;

use futures::StreamExt;
use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::sequencer::KatanaSequencer;
use katana_core::service::block_producer::BlockProducerMode;
use katana_primitives::block::BlockHashOrNumber;
use katana_provider::traits::transaction::TransactionProvider;
use katana_rpc_api::torii::ToriiApiServer;
use katana_rpc_types::error::torii::ToriiApiError;
use katana_rpc_types::receipt::{MaybePendingTxReceipt, PendingTxReceipt};
use katana_rpc_types::transaction::{TransactionsPage, TransactionsPageCursor};
use katana_rpc_types_builder::ReceiptBuilder;
use katana_tasks::TokioTaskSpawner;

const MAX_PAGE_SIZE: usize = 100;

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
        match self
            .on_io_blocking_task(move |this| {
                let mut transactions = Vec::new();
                let mut next_cursor = cursor.clone();

                let provider = this.sequencer.backend.blockchain.provider();
                let latest_block_number =
                    this.sequencer.block_number().map_err(ToriiApiError::from)?;

                if cursor.block_number > latest_block_number + 1 {
                    return Err(ToriiApiError::BlockNotFound);
                }

                if latest_block_number >= cursor.block_number {
                    for block_number in cursor.block_number..=latest_block_number {
                        let mut block_transactions = provider
                            .transactions_by_block(BlockHashOrNumber::Num(block_number))
                            .map_err(ToriiApiError::from)?
                            .ok_or(ToriiApiError::BlockNotFound)?;

                        // If the block_number is the cursor block, slice the transactions from the
                        // txn offset
                        if block_number == cursor.block_number {
                            block_transactions = block_transactions
                                .into_iter()
                                .skip(cursor.transaction_index as usize)
                                .collect();
                        }

                        let block_transactions = block_transactions
                            .into_iter()
                            .map(|tx| {
                                let receipt = ReceiptBuilder::new(tx.hash, provider)
                                    .build()
                                    .expect("Receipt should exist for tx")
                                    .expect("Receipt should exist for tx");
                                (tx, MaybePendingTxReceipt::Receipt(receipt))
                            })
                            .collect::<Vec<_>>();

                        // Add transactions to the total and break if MAX_PAGE_SIZE is reached
                        for transaction in block_transactions {
                            transactions.push(transaction);
                            if transactions.len() >= MAX_PAGE_SIZE {
                                next_cursor.block_number = block_number;
                                next_cursor.transaction_index = MAX_PAGE_SIZE as u64;
                                return Ok(TransactionsPage { transactions, cursor: next_cursor });
                            }
                        }
                    }
                }

                if let Some(pending_state) = this.sequencer.pending_state() {
                    let remaining = MAX_PAGE_SIZE - transactions.len();

                    // If cursor is in the pending block
                    if cursor.block_number == latest_block_number + 1 {
                        let pending_transactions = pending_state
                            .executed_txs
                            .read()
                            .iter()
                            .skip(cursor.transaction_index as usize)
                            .take(remaining)
                            .map(|(tx, info)| {
                                (
                                    tx.clone(),
                                    MaybePendingTxReceipt::Pending(PendingTxReceipt::new(
                                        tx.hash,
                                        info.receipt.clone(),
                                    )),
                                )
                            })
                            .collect::<Vec<_>>();

                        // If there are no transactions after the index in the pending block
                        if pending_transactions.is_empty() {
                            // Wait for a new transaction to be executed
                            let inner = this.sequencer.block_producer().inner.read();
                            let block_producer = match &*inner {
                                BlockProducerMode::Interval(block_producer) => block_producer,
                                _ => panic!(
                                    "Expected BlockProducerMode::Interval, found something else"
                                ),
                            };

                            return Err(ToriiApiError::TransactionsNotReady {
                                rx: block_producer.add_listener(),
                                cursor: next_cursor,
                            });
                        }

                        next_cursor.transaction_index += pending_transactions.len() as u64;
                        transactions.extend(pending_transactions);
                    } else {
                        let pending_transactions = pending_state
                            .executed_txs
                            .read()
                            .iter()
                            .take(remaining)
                            .map(|(tx, info)| {
                                (
                                    tx.clone(),
                                    MaybePendingTxReceipt::Pending(PendingTxReceipt::new(
                                        tx.hash,
                                        info.receipt.clone(),
                                    )),
                                )
                            })
                            .collect::<Vec<_>>();
                        next_cursor.block_number += 1;
                        next_cursor.transaction_index = pending_transactions.len() as u64;
                        transactions.extend(pending_transactions);
                    };
                } else {
                    // If there is no pending state, we are instant mining.
                    next_cursor.block_number += 1;
                    next_cursor.transaction_index = 0;

                    if transactions.is_empty() {
                        // Wait for a new transaction to be executed
                        let inner = this.sequencer.block_producer().inner.read();
                        let block_producer = match &*inner {
                            BlockProducerMode::Instant(block_producer) => block_producer,
                            _ => {
                                panic!("Expected BlockProducerMode::Instant, found something else")
                            }
                        };

                        return Err(ToriiApiError::TransactionsNotReady {
                            rx: block_producer.add_listener(),
                            cursor: next_cursor,
                        });
                    }
                }

                Ok(TransactionsPage { transactions, cursor: next_cursor })
            })
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => match e {
                ToriiApiError::TransactionsNotReady { mut rx, cursor } => {
                    let transactions = rx
                        .next()
                        .await
                        .ok_or(ToriiApiError::ChannelDisconnected)?
                        .into_iter()
                        .map(|(tx, receipt)| {
                            (
                                tx.clone(),
                                MaybePendingTxReceipt::Pending(PendingTxReceipt::new(
                                    tx.hash, receipt,
                                )),
                            )
                        })
                        .collect::<Vec<_>>();
                    let mut next_cursor = cursor;
                    next_cursor.transaction_index += transactions.len() as u64;
                    Ok(TransactionsPage { transactions, cursor: next_cursor })
                }
                _ => Err(e.into()),
            },
        }
    }
}
