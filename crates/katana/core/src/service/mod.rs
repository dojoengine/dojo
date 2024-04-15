//! background service

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::channel::mpsc::Receiver;
use futures::stream::{Fuse, Stream, StreamExt};
use katana_executor::ExecutorFactory;
use katana_primitives::transaction::ExecutableTxWithHash;
use starknet::core::types::FieldElement;
use tracing::{error, info};

use self::block_producer::BlockProducer;
use self::metrics::{BlockProducerMetrics, ServiceMetrics};
use crate::pool::TransactionPool;

pub mod block_producer;
#[cfg(feature = "messaging")]
pub mod messaging;
mod metrics;

#[cfg(feature = "messaging")]
use self::messaging::{MessagingOutcome, MessagingService};

pub(crate) const LOG_TARGET: &str = "node";

/// The type that drives the blockchain's state
///
/// This service is basically an endless future that continuously polls the miner which returns
/// transactions for the next block, then those transactions are handed off to the [BlockProducer]
/// to construct a new block.
pub struct NodeService<EF: ExecutorFactory> {
    /// the pool that holds all transactions
    pub(crate) pool: Arc<TransactionPool>,
    /// creates new blocks
    pub(crate) block_producer: Arc<BlockProducer<EF>>,
    /// the miner responsible to select transactions from the `pool´
    pub(crate) miner: TransactionMiner,
    /// The messaging service
    #[cfg(feature = "messaging")]
    pub(crate) messaging: Option<MessagingService<EF>>,
    /// Metrics for recording the service operations
    metrics: ServiceMetrics,
}

impl<EF: ExecutorFactory> NodeService<EF> {
    pub fn new(
        pool: Arc<TransactionPool>,
        miner: TransactionMiner,
        block_producer: Arc<BlockProducer<EF>>,
        #[cfg(feature = "messaging")] messaging: Option<MessagingService<EF>>,
    ) -> Self {
        let metrics = ServiceMetrics { block_producer: BlockProducerMetrics::default() };

        Self {
            pool,
            miner,
            block_producer,
            metrics,
            #[cfg(feature = "messaging")]
            messaging,
        }
    }
}

impl<EF: ExecutorFactory> Future for NodeService<EF> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();

        #[cfg(feature = "messaging")]
        if let Some(messaging) = pin.messaging.as_mut() {
            while let Poll::Ready(Some(outcome)) = messaging.poll_next_unpin(cx) {
                match outcome {
                    MessagingOutcome::Gather { msg_count, .. } => {
                        info!(target: LOG_TARGET, msg_count = %msg_count, "Collected messages from settlement chain.");
                    }
                    MessagingOutcome::Send { msg_count, .. } => {
                        info!(target: LOG_TARGET,  msg_count = %msg_count, "Sent messages to the settlement chain.");
                    }
                }
            }
        }

        // this drives block production and feeds new sets of ready transactions to the block
        // producer
        loop {
            while let Poll::Ready(Some(res)) = pin.block_producer.poll_next(cx) {
                match res {
                    Ok(outcome) => {
                        info!(target: LOG_TARGET, block_number = %outcome.block_number, "Mined block.");

                        let metrics = &pin.metrics.block_producer;
                        let gas_used = outcome.stats.l1_gas_used;
                        let steps_used = outcome.stats.cairo_steps_used;
                        metrics.l1_gas_processed_total.increment(gas_used as u64);
                        metrics.cairo_steps_processed_total.increment(steps_used as u64);
                    }

                    Err(err) => {
                        error!(target: LOG_TARGET, error = %err, "Mining block.");
                    }
                }
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
    ) -> Poll<Vec<ExecutableTxWithHash>> {
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
