// Code adapted from Foundry's Anvil

//! background service

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::channel::mpsc::Receiver;
use futures::stream::{Fuse, Stream, StreamExt};
use starknet::core::types::FieldElement;
use tracing::trace;

use self::block_producer::BlockProducer;
use crate::backend::storage::transaction::Transaction;
use crate::pool::TransactionPool;

pub mod block_producer;

/// The type that drives the blockchain's state
///
/// This service is basically an endless future that continuously polls the miner which returns
/// transactions for the next block, then those transactions are handed off to the [BlockProducer]
/// to construct a new block.
pub struct NodeService {
    /// the pool that holds all transactions
    pool: Arc<TransactionPool>,
    /// creates new blocks
    block_producer: BlockProducer,
    /// the miner responsible to select transactions from the `pool´
    miner: TransactionMiner,
}

impl NodeService {
    pub fn new(
        pool: Arc<TransactionPool>,
        miner: TransactionMiner,
        block_producer: BlockProducer,
    ) -> Self {
        Self { pool, block_producer, miner }
    }
}

impl Future for NodeService {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();

        // this drives block production and feeds new sets of ready transactions to the block
        // producer
        loop {
            while let Poll::Ready(Some(outcome)) = pin.block_producer.poll_next_unpin(cx) {
                trace!(target: "node", "mined block {}", outcome.block_number);
            }

            if let Poll::Ready(transactions) = pin.miner.poll(&pin.pool, cx) {
                // miner returned a set of transaction that we feed to the producer
                pin.block_producer.queue(transactions);
            } else {
                // no progress made
                break;
            }
        }

        Poll::Pending
    }
}

/// The type which takes the transaction from the pool and feeds them to the block producer.
pub struct TransactionMiner {
    /// stores whether there are pending transacions (if known)
    has_pending_txs: Option<bool>,
    /// Receives hashes of transactions that are ready from the pool
    rx: Fuse<Receiver<FieldElement>>,
}

impl TransactionMiner {
    pub fn new(rx: Receiver<FieldElement>) -> Self {
        Self { rx: rx.fuse(), has_pending_txs: None }
    }

    fn poll(
        &mut self,
        pool: &Arc<TransactionPool>,
        cx: &mut Context<'_>,
    ) -> Poll<Vec<Transaction>> {
        // drain the notification stream
        while let Poll::Ready(Some(_)) = Pin::new(&mut self.rx).poll_next(cx) {
            self.has_pending_txs = Some(true);
        }

        if self.has_pending_txs == Some(false) {
            return Poll::Pending;
        }

        // take all the transactions from the pool
        let transactions = pool.get_transactions();

        if transactions.is_empty() {
            return Poll::Pending;
        }

        Poll::Ready(transactions)
    }
}
