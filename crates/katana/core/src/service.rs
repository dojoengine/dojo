// Code adapted from Foundry's Anvil

//! background service

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use blockifier::state::state_api::{State, StateReader};
use futures::channel::mpsc::Receiver;
use futures::stream::{Fuse, Stream, StreamExt};
use futures::FutureExt;
use parking_lot::RwLock;
use starknet::core::types::FieldElement;
use tokio::time::{interval_at, Instant, Interval};
use tracing::trace;

use crate::backend::storage::transaction::{RejectedTransaction, Transaction};
use crate::backend::Backend;
use crate::db::cached::CachedStateWrapper;
use crate::db::StateRefDb;
use crate::execution::{
    create_execution_outcome, ExecutedTransaction, ExecutionOutcome,
    MaybeInvalidExecutedTransaction, PendingState, TransactionExecutor,
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
type InstantBlockMiningFuture = ServiceFuture<MinedBlockOutcome>;
type PendingBlockMiningFuture = ServiceFuture<(MinedBlockOutcome, StateRefDb)>;

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
#[derive(Clone)]
pub struct BlockProducer {
    /// The inner mode of mining.
    pub inner: Arc<RwLock<BlockProducerMode>>,
}

impl BlockProducer {
    /// Creates a block producer that mines a new block every `interval` milliseconds.
    pub fn interval(backend: Arc<Backend>, initial_state: StateRefDb, interval: u64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockProducerMode::Interval(PendingBlockProducer::new(
                backend,
                initial_state,
                interval,
            )))),
        }
    }

    /// Creates a new block producer that will only be possible to mine by calling the
    /// `katana_generateBlock` RPC method.
    pub fn on_demand(backend: Arc<Backend>, initial_state: StateRefDb) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockProducerMode::Interval(
                PendingBlockProducer::new_no_mining(backend, initial_state),
            ))),
        }
    }

    /// Creates a block producer that mines a new block as soon as there are ready transactions in
    /// the transactions pool.
    pub fn instant(backend: Arc<Backend>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockProducerMode::Instant(InstantBlockProducer::new(
                backend,
            )))),
        }
    }

    fn queue(&self, transactions: Vec<Transaction>) {
        let mut mode = self.inner.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.queued.push_back(transactions),
            BlockProducerMode::Interval(producer) => producer.queued.push_back(transactions),
        }
    }

    /// Returns `true` if the block producer is running in _interval_ mode. Otherwise, `fales`.
    pub fn is_interval_mining(&self) -> bool {
        matches!(*self.inner.read(), BlockProducerMode::Interval(_))
    }

    /// Returns `true` if the block producer is running in _instant_ mode. Otherwise, `fales`.
    pub fn is_instant_mining(&self) -> bool {
        matches!(*self.inner.read(), BlockProducerMode::Instant(_))
    }

    // Handler for the `katana_generateBlock` RPC method.
    pub fn force_mine(&self) {
        trace!(target: "miner", "force mining");
        let mut mode = self.inner.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => {
                tokio::task::block_in_place(|| futures::executor::block_on(producer.force_mine()))
            }
            BlockProducerMode::Interval(producer) => {
                tokio::task::block_in_place(|| futures::executor::block_on(producer.force_mine()))
            }
        }
    }
}

impl Stream for BlockProducer {
    type Item = MinedBlockOutcome;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut mode = self.inner.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.poll_next_unpin(cx),
            BlockProducerMode::Interval(producer) => producer.poll_next_unpin(cx),
        }
    }
}

pub enum BlockProducerMode {
    Interval(PendingBlockProducer),
    Instant(InstantBlockProducer),
}

pub struct PendingBlockProducer {
    /// The interval at which new blocks are mined.
    interval: Option<Interval>,
    backend: Arc<Backend>,
    /// Single active future that mines a new block
    block_mining: Option<PendingBlockMiningFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<Transaction>>,
    /// The state of the pending block after executing all the transactions within the interval.
    state: Arc<PendingState>,
    /// This is to make sure that the block context is updated
    /// before the first block is opened.
    is_initialized: bool,
}

impl PendingBlockProducer {
    pub fn new(backend: Arc<Backend>, db: StateRefDb, interval: u64) -> Self {
        let interval = Duration::from_millis(interval);
        let state = Arc::new(PendingState {
            state: RwLock::new(CachedStateWrapper::new(db)),
            executed_transactions: Default::default(),
        });

        Self {
            state,
            backend,
            block_mining: None,
            is_initialized: false,
            queued: VecDeque::default(),
            interval: Some(interval_at(Instant::now() + interval, interval)),
        }
    }

    /// Creates a new [PendingBlockProducer] with no `interval`. This mode will not produce blocks
    /// for every fixed interval, although it will still execute all queued transactions and
    /// keep hold of the pending state.
    pub fn new_no_mining(backend: Arc<Backend>, db: StateRefDb) -> Self {
        let state = Arc::new(PendingState {
            state: RwLock::new(CachedStateWrapper::new(db)),
            executed_transactions: Default::default(),
        });

        Self {
            state,
            backend,
            block_mining: None,
            is_initialized: false,
            queued: VecDeque::default(),
            interval: None,
        }
    }

    pub fn state(&self) -> Arc<PendingState> {
        self.state.clone()
    }

    /// Force mine a new block. It will only able to mine if there is no ongoing mining process.
    pub async fn force_mine(&self) {
        if self.block_mining.is_none() {
            let outcome = self.outcome();
            let (_, new_state) = Self::do_mine(outcome, self.backend.clone()).await;
            self.reset(new_state);
        } else {
            trace!(target: "miner", "unable to force mine while a mining process is running")
        }
    }

    async fn do_mine(
        execution_outcome: ExecutionOutcome,
        backend: Arc<Backend>,
    ) -> (MinedBlockOutcome, StateRefDb) {
        trace!(target: "miner", "creating new block");
        let (outcome, new_state) = backend.mine_pending_block(execution_outcome).await;
        trace!(target: "miner", "created new block: {}", outcome.block_number);
        backend.update_block_context();
        (outcome, new_state)
    }

    fn reset(&self, state: StateRefDb) {
        self.state.executed_transactions.write().clear();
        *self.state.state.write() = CachedStateWrapper::new(state);
    }

    fn execute_transactions(
        transactions: Vec<Transaction>,
        state: Arc<PendingState>,
        backend: Arc<Backend>,
    ) {
        let transactions = {
            let mut state = state.state.write();
            TransactionExecutor::new(
                &mut state,
                &backend.env.read().block,
                !backend.config.read().disable_fee,
            )
            .with_error_log()
            .with_events_log()
            .with_resources_log()
            .execute_many(transactions.clone())
            .into_iter()
            .zip(transactions)
            .map(|(res, tx)| match res {
                Ok(exec_info) => {
                    let executed_tx = ExecutedTransaction::new(tx, exec_info);
                    MaybeInvalidExecutedTransaction::Valid(Arc::new(executed_tx))
                }
                Err(err) => {
                    let rejected_tx =
                        RejectedTransaction { inner: tx, execution_error: err.to_string() };
                    MaybeInvalidExecutedTransaction::Invalid(Arc::new(rejected_tx))
                }
            })
            .collect::<Vec<_>>()
        };

        state.executed_transactions.write().extend(transactions)
    }

    fn outcome(&self) -> ExecutionOutcome {
        let state = &mut self.state.state.write();

        let declared_sierra_classes = state.sierra_class().clone();
        let state_diff = state.to_state_diff();
        let declared_classes = state_diff
            .class_hash_to_compiled_class_hash
            .iter()
            .map(|(class_hash, _)| {
                let contract_class = state
                    .get_compiled_contract_class(class_hash)
                    .expect("contract class must exist in state if declared");
                (*class_hash, contract_class)
            })
            .collect::<HashMap<_, _>>();

        ExecutionOutcome {
            state_diff,
            declared_classes,
            declared_sierra_classes,
            transactions: self.state.executed_transactions.read().clone(),
        }
    }
}

impl Stream for PendingBlockProducer {
    // mined block outcome and the new state
    type Item = MinedBlockOutcome;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if !pin.is_initialized {
            pin.backend.update_block_context();
            pin.is_initialized = true;
        }

        if let Some(interval) = &mut pin.interval {
            if interval.poll_tick(cx).is_ready() && pin.block_mining.is_none() {
                pin.block_mining =
                    Some(Box::pin(Self::do_mine(pin.outcome(), pin.backend.clone())));
            }
        }

        // only execute transactions if there is no mining in progress
        if !pin.queued.is_empty() && pin.block_mining.is_none() {
            let transactions = pin.queued.pop_front().expect("not empty; qed");
            Self::execute_transactions(transactions, pin.state.clone(), pin.backend.clone());
        }

        // poll the mining future
        if let Some(mut mining) = pin.block_mining.take() {
            // reset the executor for the next block
            if let Poll::Ready((outcome, new_state)) = mining.poll_unpin(cx) {
                // update the block context for the next pending block
                pin.reset(new_state);
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
    backend: Arc<Backend>,
    /// Single active future that mines a new block
    block_mining: Option<InstantBlockMiningFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<Transaction>>,
}

impl InstantBlockProducer {
    pub fn new(backend: Arc<Backend>) -> Self {
        Self { backend, block_mining: None, queued: VecDeque::default() }
    }

    pub async fn force_mine(&mut self) {
        if self.block_mining.is_none() {
            let txs = self.queued.pop_front().unwrap_or_default();
            let _ = Self::do_mine(self.backend.clone(), txs).await;
        } else {
            trace!(target: "miner", "unable to force mine while a mining process is running")
        }
    }

    async fn do_mine(backend: Arc<Backend>, transactions: Vec<Transaction>) -> MinedBlockOutcome {
        trace!(target: "miner", "creating new block");

        backend.update_block_context();

        let mut state = CachedStateWrapper::new(backend.state.read().await.as_ref_db());
        let block_context = backend.env.read().block.clone();

        let results = TransactionExecutor::new(
            &mut state,
            &block_context,
            !backend.config.read().disable_fee,
        )
        .with_error_log()
        .with_events_log()
        .with_resources_log()
        .execute_many(transactions.clone());

        let outcome = backend
            .do_mine_block(create_execution_outcome(
                &mut state,
                transactions.into_iter().zip(results).collect(),
            ))
            .await;

        trace!(target: "miner", "created new block: {}", outcome.block_number);

        outcome
    }
}

impl Stream for InstantBlockProducer {
    // mined block outcome and the new state
    type Item = MinedBlockOutcome;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if !pin.queued.is_empty() && pin.block_mining.is_none() {
            let transactions = pin.queued.pop_front().expect("not empty; qed");
            pin.block_mining = Some(Box::pin(Self::do_mine(pin.backend.clone(), transactions)));
        }

        // poll the mining future
        if let Some(mut mining) = pin.block_mining.take() {
            if let Poll::Ready(outcome) = mining.poll_unpin(cx) {
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
