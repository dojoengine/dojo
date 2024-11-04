// TODO: remove the messaging feature flag
// TODO: move the tasks to a separate module

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use block_producer::BlockProductionError;
use futures::channel::mpsc::Receiver;
use futures::stream::{Fuse, Stream, StreamExt};
use katana_executor::ExecutorFactory;
use katana_pool::ordering::PoolOrd;
use katana_pool::pending::PendingTransactions;
use katana_pool::{TransactionPool, TxPool};
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_primitives::Felt;
use tracing::{error, info};

use self::block_producer::BlockProducer;
use self::metrics::BlockProducerMetrics;

pub mod block_producer;
pub mod messaging;
mod metrics;

pub(crate) const LOG_TARGET: &str = "node";

/// The type that drives the blockchain's state
///
/// This task is basically an endless future that continuously polls the miner which returns
/// transactions for the next block, then those transactions are handed off to the [BlockProducer]
/// to construct a new block.
#[must_use = "BlockProductionTask does nothing unless polled"]
#[allow(missing_debug_implementations)]
pub struct BlockProductionTask<EF, O>
where
    EF: ExecutorFactory,
    O: PoolOrd<Transaction = ExecutableTxWithHash>,
{
    /// creates new blocks
    pub(crate) block_producer: BlockProducer<EF>,
    /// the miner responsible to select transactions from the `pool´
    pub(crate) miner: TransactionMiner<O>,
    /// the pool that holds all transactions
    pub(crate) pool: TxPool,
    /// Metrics for recording the service operations
    metrics: BlockProducerMetrics,
}

impl<EF, O> BlockProductionTask<EF, O>
where
    EF: ExecutorFactory,
    O: PoolOrd<Transaction = ExecutableTxWithHash>,
{
    pub fn new(
        pool: TxPool,
        miner: TransactionMiner<O>,
        block_producer: BlockProducer<EF>,
    ) -> Self {
        Self { block_producer, miner, pool, metrics: BlockProducerMetrics::default() }
    }
}

impl<EF, O> Future for BlockProductionTask<EF, O>
where
    EF: ExecutorFactory,
    O: PoolOrd<Transaction = ExecutableTxWithHash>,
{
    type Output = Result<(), BlockProductionError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // this drives block production and feeds new sets of ready transactions to the block
        // producer
        loop {
            while let Poll::Ready(Some(res)) = this.block_producer.poll_next(cx) {
                match res {
                    Ok(outcome) => {
                        info!(target: LOG_TARGET, block_number = %outcome.block_number, "Mined block.");

                        let gas_used = outcome.stats.l1_gas_used;
                        let steps_used = outcome.stats.cairo_steps_used;
                        this.metrics.l1_gas_processed_total.increment(gas_used as u64);
                        this.metrics.cairo_steps_processed_total.increment(steps_used as u64);

                        // remove mined transactions from the pool
                        this.pool.remove_transactions(&outcome.txs);
                    }

                    Err(error) => {
                        error!(target: LOG_TARGET, %error, "Mining block.");
                        return Poll::Ready(Err(error));
                    }
                }
            }

            if let Poll::Ready(pool_txs) = this.miner.poll(cx) {
                // miner returned a set of transaction that we feed to the producer
                this.block_producer.queue(pool_txs);
            } else {
                // no progress made
                break;
            }
        }

        Poll::Pending
    }
}

/// The type which takes the transaction from the pool and feeds them to the block producer.
#[derive(Debug)]
pub struct TransactionMiner<O>
where
    O: PoolOrd<Transaction = ExecutableTxWithHash>,
{
    /// stores whether there are pending transacions (if known)
    has_pending_txs: Option<bool>,
    /// Receives hashes of transactions that are ready from the pool
    rx: Fuse<Receiver<Felt>>,

    pending_txs: PendingTransactions<ExecutableTxWithHash, O>,
}

impl<O> TransactionMiner<O>
where
    O: PoolOrd<Transaction = ExecutableTxWithHash>,
{
    pub fn new(
        pending_txs: PendingTransactions<ExecutableTxWithHash, O>,
        rx: Receiver<Felt>,
    ) -> Self {
        Self { pending_txs, rx: rx.fuse(), has_pending_txs: None }
    }

    fn poll(
        &mut self,
        // pool: &TxPool,
        cx: &mut Context<'_>,
    ) -> Poll<Vec<ExecutableTxWithHash>> {
        // drain the notification stream
        while let Poll::Ready(Some(_)) = Pin::new(&mut self.rx).poll_next(cx) {
            self.has_pending_txs = Some(true);
        }

        if self.has_pending_txs == Some(false) {
            return Poll::Pending;
        }

        let mut transactions = Vec::new();
        while let Poll::Ready(Some(tx)) = self.pending_txs.poll_next_unpin(cx) {
            transactions.push(tx.tx.as_ref().clone());
        }

        // take all the transactions from the pool
        // let transactions =
        //     pool.take_transactions().map(|tx| tx.tx.as_ref().clone()).collect::<Vec<_>>();

        if transactions.is_empty() {
            return Poll::Pending;
        }

        self.has_pending_txs = Some(false);
        Poll::Ready(transactions)
    }
}
