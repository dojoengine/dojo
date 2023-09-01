// Code adapted from Foundry's Anvil

//! background service

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::mpsc::Receiver;
use futures::stream::{Fuse, Stream, StreamExt};
use futures::FutureExt;
use starknet::core::types::FieldElement;
use tokio::time::{Instant, Interval};
use tracing::trace;

use crate::backend::storage::transaction::Transaction;
use crate::backend::Backend;
use crate::db::cached::CachedStateWrapper;
use crate::db::StateRefDb;
use crate::execution::{
    create_execution_outcome, ExecutionOutcome, MaybeInvalidExecutedTransaction,
    PendingBlockExecutor, TransactionExecutor,
};
use crate::pool::TransactionPool;

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

type ServiceFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync>>;
type InstantBlockMiningFuture = ServiceFuture<(MinedBlockOutcome, Arc<Backend>)>;
type PendingBlockMiningFuture = ServiceFuture<(MinedBlockOutcome, Arc<Backend>, StateRefDb)>;

/// The type which responsible for block production.
///
/// On _interval_ mining, a new block is opened for a fixed amount of interval. Within this
/// interval, it executes all the queued transactions and keep hold of the pending state after
/// executing all the transactions. Once the interval is over, the block producer will close/mine
/// the block with all the transactions that have been executed within the interval and applies the
/// resulting state to the latest state. Then, a new block is opened for the next interval. As such,
/// the block context is updated only when a new block is opened.
///
/// On _instant_ mining, a new block is mined as soon as there are transactions in the tx pool. The
/// block producer will execute all the transactions in the mempool and mine a new block with the
/// resulting state. The block context is only updated every time a new block is mined as opposed to
/// updating it when the block is opened (in _interval_ mode).
#[must_use = "BlockProducer does nothing unless polled"]
pub enum BlockProducer {
    Interval(PendingBlockProducer),
    Instant(InstantBlockProducer),
}

impl BlockProducer {
    /// Creates a block producer that mines a new block every `interval` milliseconds.
    pub fn interval(backend: Arc<Backend>, executor: PendingBlockExecutor, interval: u64) -> Self {
        Self::Interval(PendingBlockProducer::new(backend, executor, interval))
    }

    /// Creates a block producer that mines a new block as soon as there are ready transactions in
    /// the transactions pool.
    pub fn instant(backend: Arc<Backend>) -> Self {
        Self::Instant(InstantBlockProducer::new(backend))
    }

    fn queue(&mut self, transactions: Vec<Transaction>) {
        match self {
            Self::Instant(producer) => producer.queued.push_back(transactions),
            Self::Interval(producer) => producer.queued.push_back(transactions),
        }
    }

    pub fn is_interval_mining(&self) -> bool {
        matches!(self, Self::Interval(_))
    }

    pub fn is_instant_mining(&self) -> bool {
        matches!(self, Self::Instant(_))
    }
}

impl Stream for BlockProducer {
    type Item = MinedBlockOutcome;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.get_mut() {
            Self::Instant(producer) => producer.poll_next_unpin(cx),
            Self::Interval(producer) => producer.poll_next_unpin(cx),
        }
    }
}

pub struct PendingBlockProducer {
    /// The interval at which new blocks are mined.
    interval: Interval,
    /// Holds the backend if no block is being mined
    idle_backend: Option<Arc<Backend>>,
    /// Single active future that mines a new block
    block_mining: Option<PendingBlockMiningFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<Transaction>>,

    /// Executor which executes transactions
    executor: PendingBlockExecutor,

    /// This is to make sure that the block context is updated
    /// before the first block is opened.
    is_initialized: bool,
}

impl PendingBlockProducer {
    pub fn new(backend: Arc<Backend>, executor: PendingBlockExecutor, interval: u64) -> Self {
        let interval = Duration::from_millis(interval);
        Self {
            executor,
            block_mining: None,
            is_initialized: false,
            idle_backend: Some(backend),
            queued: VecDeque::default(),
            interval: tokio::time::interval_at(Instant::now() + interval, interval),
        }
    }

    async fn do_mine(
        execution_outcome: ExecutionOutcome,
        backend: Arc<Backend>,
    ) -> (MinedBlockOutcome, Arc<Backend>, StateRefDb) {
        trace!(target: "miner", "creating new block");
        let (outcome, new_state) = backend.mine_pending_block(execution_outcome).await;
        trace!(target: "miner", "created new block: {}", outcome.block_number);
        (outcome, backend, new_state)
    }
}

impl Stream for PendingBlockProducer {
    // mined block outcome and the new state
    type Item = MinedBlockOutcome;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        match &pin.idle_backend {
            Some(backend) if !pin.is_initialized => {
                backend.update_block_context();
                pin.is_initialized = true;
            }

            _ => {}
        }

        if pin.interval.poll_tick(cx).is_ready() {
            if let Some(backend) = pin.idle_backend.take() {
                let execution_outcome = pin.executor.outcome();
                pin.block_mining = Some(Box::pin(Self::do_mine(execution_outcome, backend)));
            }
        }

        // only execute transactions if there is no mining in progress
        if !pin.queued.is_empty() && pin.block_mining.is_none() {
            let transactions = pin.queued.pop_front().expect("not empty; qed");
            pin.executor.execute(transactions);
        }

        // poll the mining future
        if let Some(mut mining) = pin.block_mining.take() {
            // reset the executor for the next block
            if let Poll::Ready((outcome, backend, new_state)) = mining.poll_unpin(cx) {
                // update the block context for the next pending block
                backend.update_block_context();

                pin.executor.reset(new_state);
                pin.idle_backend = Some(backend);

                return Poll::Ready(Some(outcome));
            } else {
                pin.block_mining = Some(mining)
            }
        }

        Poll::Pending
    }
}

pub struct InstantBlockProducer {
    /// Holds the backend if no block is being mined
    idle_backend: Option<Arc<Backend>>,
    /// Single active future that mines a new block
    block_mining: Option<InstantBlockMiningFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<Transaction>>,
}

impl InstantBlockProducer {
    pub fn new(backend: Arc<Backend>) -> Self {
        Self { block_mining: None, idle_backend: Some(backend), queued: VecDeque::default() }
    }

    async fn do_mine(
        backend: Arc<Backend>,
        transactions: Vec<Transaction>,
    ) -> (MinedBlockOutcome, Arc<Backend>) {
        trace!(target: "miner", "creating new block");

        backend.update_block_context();

        let mut state = CachedStateWrapper::new(backend.state.read().await.as_ref_db());
        let block_context = backend.env.read().block.clone();

        let results = TransactionExecutor::new(&mut state, &block_context, true)
            .execute_many(transactions.clone());

        let outcome = backend
            .do_mine_block(create_execution_outcome(
                &mut state,
                transactions.into_iter().zip(results).collect(),
            ))
            .await;

        trace!(target: "miner", "created new block: {}", outcome.block_number);

        (outcome, backend)
    }
}

impl Stream for InstantBlockProducer {
    // mined block outcome and the new state
    type Item = MinedBlockOutcome;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if !pin.queued.is_empty() {
            if let Some(backend) = pin.idle_backend.take() {
                let transactions = pin.queued.pop_front().expect("not empty; qed");
                pin.block_mining = Some(Box::pin(Self::do_mine(backend, transactions)));
            }
        }

        // poll the mining future
        if let Some(mut mining) = pin.block_mining.take() {
            if let Poll::Ready((outcome, backend)) = mining.poll_unpin(cx) {
                pin.idle_backend = Some(backend);
                return Poll::Ready(Some(outcome));
            } else {
                pin.block_mining = Some(mining)
            }
        }

        Poll::Pending
    }
}

pub struct MinedBlockOutcome {
    pub block_number: u64,
    pub transactions: Vec<MaybeInvalidExecutedTransaction>,
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
