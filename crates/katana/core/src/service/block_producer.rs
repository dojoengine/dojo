use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::{Stream, StreamExt};
use futures::FutureExt;
use katana_executor::{BlockExecutor, ExecutionOutput, ExecutorFactory};
use katana_primitives::block::{BlockHashOrNumber, ExecutableBlock, PartialHeader};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::{ExecutableTxWithHash, TxWithHash};
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_provider::error::ProviderError;
use katana_provider::traits::block::{BlockHashProvider, BlockNumberProvider};
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::StateFactoryProvider;
use katana_tasks::{BlockingTaskPool, BlockingTaskResult};
use parking_lot::RwLock;
use tokio::time::{interval_at, Instant, Interval};
use tracing::{error, info, trace, warn};

use crate::backend::Backend;

#[derive(Debug, thiserror::Error)]
pub enum BlockProductionError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("block mining task cancelled")]
    BlockMiningTaskCancelled,

    #[error("transaction execution task cancelled")]
    ExecutionTaskCancelled,

    #[error("transaction execution error: {0}")]
    TransactionExecutionError(#[from] katana_executor::ExecutorError),
}

pub struct MinedBlockOutcome {
    pub block_number: u64,
}

type ServiceFuture<T> = Pin<Box<dyn Future<Output = BlockingTaskResult<T>> + Send + Sync>>;

type BlockProductionResult = Result<MinedBlockOutcome, BlockProductionError>;
type BlockProductionFuture = ServiceFuture<BlockProductionResult>;

type TxExecutionResult = Result<TxWithHashAndReceiptPairs, BlockProductionError>;
type TxExecutionFuture = ServiceFuture<TxExecutionResult>;

type BlockProductionWithTxnsFuture =
    ServiceFuture<Result<(MinedBlockOutcome, TxWithHashAndReceiptPairs), BlockProductionError>>;
pub type TxWithHashAndReceiptPairs = Vec<(TxWithHash, Receipt)>;

/// The type which responsible for block production.
#[must_use = "BlockProducer does nothing unless polled"]
pub struct BlockProducer<EF: ExecutorFactory> {
    /// The inner mode of mining.
    pub inner: RwLock<BlockProducerMode<EF>>,
}

impl<EF: ExecutorFactory> BlockProducer<EF> {
    /// Creates a block producer that mines a new block every `interval` milliseconds.
    pub fn interval(backend: Arc<Backend<EF>>, interval: u64) -> Self {
        Self {
            inner: RwLock::new(BlockProducerMode::Interval(IntervalBlockProducer::new(
                backend, interval,
            ))),
        }
    }

    /// Creates a new block producer that will only be possible to mine by calling the
    /// `katana_generateBlock` RPC method.
    pub fn on_demand(backend: Arc<Backend<EF>>) -> Self {
        Self {
            inner: RwLock::new(BlockProducerMode::Interval(IntervalBlockProducer::new_no_mining(
                backend,
            ))),
        }
    }

    /// Creates a block producer that mines a new block as soon as there are ready transactions in
    /// the transactions pool.
    pub fn instant(backend: Arc<Backend<EF>>) -> Self {
        Self { inner: RwLock::new(BlockProducerMode::Instant(InstantBlockProducer::new(backend))) }
    }

    pub(super) fn queue(&self, transactions: Vec<ExecutableTxWithHash>) {
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
        trace!(target: "miner", "scheduling force block mining");
        let mut mode = self.inner.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.force_mine(),
            BlockProducerMode::Interval(producer) => producer.force_mine(),
        }
    }

    pub(super) fn poll_next(&self, cx: &mut Context<'_>) -> Poll<Option<BlockProductionResult>> {
        let mut mode = self.inner.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.poll_next_unpin(cx),
            BlockProducerMode::Interval(producer) => producer.poll_next_unpin(cx),
        }
    }
}

/// The inner type of [BlockProducer].
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
pub enum BlockProducerMode<EF: ExecutorFactory> {
    Interval(IntervalBlockProducer<EF>),
    Instant(InstantBlockProducer<EF>),
}

#[derive(Clone, derive_more::Deref)]
pub struct PendingExecutor(#[deref] Arc<RwLock<Box<dyn BlockExecutor<'static>>>>);

impl PendingExecutor {
    fn new(executor: Box<dyn BlockExecutor<'static>>) -> Self {
        Self(Arc::new(RwLock::new(executor)))
    }
}

pub struct IntervalBlockProducer<EF: ExecutorFactory> {
    /// The interval at which new blocks are mined.
    interval: Option<Interval>,
    backend: Arc<Backend<EF>>,
    /// Single active future that mines a new block
    ongoing_mining: Option<BlockProductionFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<ExecutableTxWithHash>>,
    executor: PendingExecutor,
    blocking_task_spawner: BlockingTaskPool,
    ongoing_execution: Option<TxExecutionFuture>,
    /// Listeners notified when a new executed tx is added.
    tx_execution_listeners: RwLock<Vec<Sender<TxWithHashAndReceiptPairs>>>,
}

impl<EF: ExecutorFactory> IntervalBlockProducer<EF> {
    pub fn new(backend: Arc<Backend<EF>>, interval: u64) -> Self {
        let interval = {
            let duration = Duration::from_millis(interval);
            let mut interval = interval_at(Instant::now() + duration, duration);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval
        };

        let provider = backend.blockchain.provider();

        let latest_num = provider.latest_number().unwrap();
        let mut block_env = provider.block_env_at(latest_num.into()).unwrap().unwrap();
        backend.update_block_env(&mut block_env);

        let state = provider.latest().unwrap();
        let executor = backend.executor_factory.with_state_and_block_env(state, block_env);
        let executor = PendingExecutor::new(executor);

        let blocking_task_spawner = BlockingTaskPool::new().unwrap();

        Self {
            backend,
            executor,
            ongoing_mining: None,
            blocking_task_spawner,
            ongoing_execution: None,
            interval: Some(interval),
            queued: VecDeque::default(),
            tx_execution_listeners: RwLock::new(vec![]),
        }
    }

    /// Creates a new [IntervalBlockProducer] with no `interval`. This mode will not produce blocks
    /// for every fixed interval, although it will still execute all queued transactions and
    /// keep hold of the pending state.
    pub fn new_no_mining(backend: Arc<Backend<EF>>) -> Self {
        let provider = backend.blockchain.provider();

        let latest_num = provider.latest_number().unwrap();
        let mut block_env = provider.block_env_at(latest_num.into()).unwrap().unwrap();
        backend.update_block_env(&mut block_env);

        let state = provider.latest().unwrap();
        let executor = backend.executor_factory.with_state_and_block_env(state, block_env);
        let executor = PendingExecutor::new(executor);

        let blocking_task_spawner = BlockingTaskPool::new().unwrap();

        Self {
            backend,
            executor,
            interval: None,
            ongoing_mining: None,
            queued: VecDeque::default(),
            blocking_task_spawner,
            ongoing_execution: None,
            tx_execution_listeners: RwLock::new(vec![]),
        }
    }

    pub fn executor(&self) -> PendingExecutor {
        self.executor.clone()
    }

    /// Force mine a new block. It will only able to mine if there is no ongoing mining process.
    pub fn force_mine(&mut self) {
        match Self::do_mine(self.executor.clone(), self.backend.clone()) {
            Ok(outcome) => {
                info!(target: "miner", "force mined block {}", outcome.block_number);
                self.executor =
                    self.create_new_executor_for_next_block().expect("fail to create executor");
            }
            Err(e) => {
                error!(target: "miner", "failed to force mine: {e}");
            }
        }
    }

    fn do_mine(
        executor: PendingExecutor,
        backend: Arc<Backend<EF>>,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        let executor = &mut executor.write();

        trace!(target: "miner", "creating new block");

        let block_env = executor.block_env();
        let ExecutionOutput { states, transactions } = executor.take_execution_output()?;

        let transactions = transactions
            .into_iter()
            .filter_map(|(tx, rct)| rct.map(|rct| (tx, rct)))
            .collect::<Vec<_>>();

        let outcome = backend.do_mine_block(&block_env, transactions, states)?;

        trace!(target: "miner", "created new block: {}", outcome.block_number);

        Ok(outcome)
    }

    fn execute_transactions(
        executor: PendingExecutor,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> Result<TxWithHashAndReceiptPairs, BlockProductionError> {
        let tx_receipt_pair = transactions
            .into_iter()
            .map(|tx| {
                let tx_ = TxWithHash::from(&tx);
                let output = executor.write().execute(tx)?;

                let receipt = output.receipt(tx_.as_ref());
                Ok((tx_, receipt))
            })
            .collect::<Result<TxWithHashAndReceiptPairs, BlockProductionError>>()?;

        Ok(tx_receipt_pair)
    }

    fn create_new_executor_for_next_block(&self) -> Result<PendingExecutor, BlockProductionError> {
        let backend = &self.backend;
        let provider = backend.blockchain.provider();

        let latest_num = provider.latest_number()?;
        let updated_state = provider.latest()?;

        let mut block_env = provider.block_env_at(latest_num.into())?.unwrap();
        backend.update_block_env(&mut block_env);

        let executor = backend.executor_factory.with_state_and_block_env(updated_state, block_env);
        Ok(PendingExecutor::new(executor))
    }

    pub fn add_listener(&self) -> Receiver<TxWithHashAndReceiptPairs> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.tx_execution_listeners.write().push(tx);
        rx
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, txs: TxWithHashAndReceiptPairs) {
        let mut listener = self.tx_execution_listeners.write();
        // this is basically a retain but with mut reference
        for n in (0..listener.len()).rev() {
            let mut listener_tx = listener.swap_remove(n);
            let retain = match listener_tx.try_send(txs.clone()) {
                Ok(()) => true,
                Err(e) => {
                    if e.is_full() {
                        warn!(
                            target: "miner",
                            "failed to send new txs notification because channel is full",
                        );
                        true
                    } else {
                        false
                    }
                }
            };
            if retain {
                listener.push(listener_tx)
            }
        }
    }
}

impl<EF: ExecutorFactory> Stream for IntervalBlockProducer<EF> {
    // mined block outcome and the new state
    type Item = BlockProductionResult;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if let Some(interval) = &mut pin.interval {
            // mine block if the interval is over
            if interval.poll_tick(cx).is_ready() && pin.ongoing_mining.is_none() {
                let executor = pin.executor.clone();
                let backend = pin.backend.clone();
                let fut = pin.blocking_task_spawner.spawn(|| Self::do_mine(executor, backend));
                pin.ongoing_mining = Some(Box::pin(fut));
            }
        }

        loop {
            if !pin.queued.is_empty()
                && pin.ongoing_execution.is_none()
                && pin.ongoing_mining.is_none()
            {
                let executor = pin.executor.clone();
                let transactions: Vec<ExecutableTxWithHash> =
                    std::mem::take(&mut pin.queued).into_iter().flatten().collect();

                let fut = pin
                    .blocking_task_spawner
                    .spawn(|| Self::execute_transactions(executor, transactions));

                pin.ongoing_execution = Some(Box::pin(fut));
            }

            // poll the ongoing execution if any
            if let Some(mut execution) = pin.ongoing_execution.take() {
                if let Poll::Ready(executor) = execution.poll_unpin(cx) {
                    match executor {
                        Ok(Ok(txs)) => {
                            pin.notify_listener(txs);
                            continue;
                        }

                        Ok(Err(e)) => {
                            return Poll::Ready(Some(Err(e)));
                        }

                        Err(_) => {
                            return Poll::Ready(Some(Err(
                                BlockProductionError::ExecutionTaskCancelled,
                            )));
                        }
                    }
                } else {
                    pin.ongoing_execution = Some(execution);
                }
            }

            break;
        }

        // poll the mining future if any
        if let Some(mut mining) = pin.ongoing_mining.take() {
            if let Poll::Ready(res) = mining.poll_unpin(cx) {
                match res {
                    Ok(outcome) => {
                        match pin.create_new_executor_for_next_block() {
                            Ok(executor) => {
                                pin.executor = executor;
                            }

                            Err(e) => return Poll::Ready(Some(Err(e))),
                        }

                        return Poll::Ready(Some(outcome));
                    }

                    Err(_) => {
                        return Poll::Ready(Some(Err(
                            BlockProductionError::BlockMiningTaskCancelled,
                        )));
                    }
                }
            } else {
                pin.ongoing_mining = Some(mining);
            }
        }

        Poll::Pending
    }
}

pub struct InstantBlockProducer<EF: ExecutorFactory> {
    /// Holds the backend if no block is being mined
    backend: Arc<Backend<EF>>,
    /// Single active future that mines a new block
    block_mining: Option<BlockProductionWithTxnsFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<ExecutableTxWithHash>>,

    blocking_task_pool: BlockingTaskPool,
    /// Listeners notified when a new executed tx is added.
    tx_execution_listeners: RwLock<Vec<Sender<TxWithHashAndReceiptPairs>>>,
}

impl<EF: ExecutorFactory> InstantBlockProducer<EF> {
    pub fn new(backend: Arc<Backend<EF>>) -> Self {
        Self {
            backend,
            block_mining: None,
            queued: VecDeque::default(),
            blocking_task_pool: BlockingTaskPool::new().unwrap(),
            tx_execution_listeners: RwLock::new(vec![]),
        }
    }

    pub fn force_mine(&mut self) {
        if self.block_mining.is_none() {
            let txs = self.queued.pop_front().unwrap_or_default();
            let _ = Self::do_mine(self.backend.clone(), txs);
        } else {
            trace!(target: "miner", "unable to force mine while a mining process is running")
        }
    }

    fn do_mine(
        backend: Arc<Backend<EF>>,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> Result<(MinedBlockOutcome, TxWithHashAndReceiptPairs), BlockProductionError> {
        trace!(target: "miner", "creating new block");

        let provider = backend.blockchain.provider();

        let latest_num = provider.latest_number()?;
        let mut block_env = provider.block_env_at(BlockHashOrNumber::Num(latest_num))?.unwrap();
        backend.update_block_env(&mut block_env);

        let parent_hash = provider.latest_hash()?;
        let latest_state = provider.latest()?;

        let mut executor = backend.executor_factory.with_state(latest_state);

        let block = ExecutableBlock {
            body: transactions,
            header: PartialHeader {
                parent_hash,
                number: block_env.number,
                timestamp: block_env.timestamp,
                gas_prices: block_env.l1_gas_prices.clone(),
                sequencer_address: block_env.sequencer_address,
                version: CURRENT_STARKNET_VERSION,
            },
        };

        executor.execute_block(block)?;

        let ExecutionOutput { states, transactions } = executor.take_execution_output().unwrap();
        let tx_receipt_pairs = transactions
            .into_iter()
            .filter_map(|(tx, rct)| rct.map(|rct| (tx, rct)))
            .collect::<Vec<_>>();

        let outcome = backend.do_mine_block(&block_env, tx_receipt_pairs.clone(), states)?;

        trace!(target: "miner", "created new block: {}", outcome.block_number);

        Ok((outcome, tx_receipt_pairs))
    }

    pub fn add_listener(&self) -> Receiver<TxWithHashAndReceiptPairs> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.tx_execution_listeners.write().push(tx);
        rx
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, txs: TxWithHashAndReceiptPairs) {
        let mut listener = self.tx_execution_listeners.write();
        // this is basically a retain but with mut reference
        for n in (0..listener.len()).rev() {
            let mut listener_tx = listener.swap_remove(n);
            let retain = match listener_tx.try_send(txs.clone()) {
                Ok(()) => true,
                Err(e) => {
                    if e.is_full() {
                        warn!(
                            target: "miner",
                            "failed to send new txs notification because channel is full",
                        );
                        true
                    } else {
                        false
                    }
                }
            };
            if retain {
                listener.push(listener_tx)
            }
        }
    }
}

impl<EF: ExecutorFactory> Stream for InstantBlockProducer<EF> {
    // mined block outcome and the new state
    type Item = Result<MinedBlockOutcome, BlockProductionError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if !pin.queued.is_empty() && pin.block_mining.is_none() {
            let transactions = pin.queued.pop_front().expect("not empty; qed");
            let backend = pin.backend.clone();

            pin.block_mining = Some(Box::pin(
                pin.blocking_task_pool.spawn(|| Self::do_mine(backend, transactions)),
            ));
        }

        // poll the mining future
        if let Some(mut mining) = pin.block_mining.take() {
            if let Poll::Ready(outcome) = mining.poll_unpin(cx) {
                match outcome {
                    Ok(Ok((outcome, txs))) => {
                        pin.notify_listener(txs);
                        return Poll::Ready(Some(Ok(outcome)));
                    }

                    Ok(Err(e)) => {
                        return Poll::Ready(Some(Err(e)));
                    }

                    Err(_) => {
                        return Poll::Ready(Some(Err(
                            BlockProductionError::ExecutionTaskCancelled,
                        )));
                    }
                }
            } else {
                pin.block_mining = Some(mining)
            }
        }

        Poll::Pending
    }
}

