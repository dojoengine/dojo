use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::{Stream, StreamExt};
use futures::FutureExt;
use katana_executor::{BlockExecutor, ExecutionResult, ExecutionStats, ExecutorFactory};
use katana_pool::validation::stateful::TxValidator;
use katana_primitives::block::{BlockHashOrNumber, ExecutableBlock, PartialHeader};
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::receipt::Receipt;
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{ExecutableTxWithHash, TxHash, TxWithHash};
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_provider::error::ProviderError;
use katana_provider::traits::block::{BlockHashProvider, BlockNumberProvider};
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::StateFactoryProvider;
use katana_tasks::{BlockingTaskPool, BlockingTaskResult};
use parking_lot::lock_api::RawMutex;
use parking_lot::{Mutex, RwLock};
use tokio::time::{interval_at, Instant, Interval};
use tracing::{error, info, trace, warn};

use crate::backend::Backend;

pub(crate) const LOG_TARGET: &str = "miner";

#[derive(Debug, thiserror::Error)]
pub enum BlockProductionError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("transaction execution task cancelled")]
    ExecutionTaskCancelled,

    #[error("transaction execution error: {0}")]
    TransactionExecutionError(#[from] katana_executor::ExecutorError),
}

#[derive(Debug, Clone)]
pub struct MinedBlockOutcome {
    pub block_number: u64,
    pub txs: Vec<TxHash>,
    pub stats: ExecutionStats,
}

#[derive(Debug, Clone)]
pub struct TxWithOutcome {
    pub tx: TxWithHash,
    pub receipt: Receipt,
    pub exec_info: TxExecInfo,
}

type ServiceFuture<T> = Pin<Box<dyn Future<Output = BlockingTaskResult<T>> + Send + Sync>>;

type BlockProductionResult = Result<MinedBlockOutcome, BlockProductionError>;
type BlockProductionFuture = ServiceFuture<Result<MinedBlockOutcome, BlockProductionError>>;

type TxExecutionResult = Result<Vec<TxWithOutcome>, BlockProductionError>;
type TxExecutionFuture = ServiceFuture<TxExecutionResult>;

type BlockProductionWithTxnsFuture =
    ServiceFuture<Result<(MinedBlockOutcome, Vec<TxWithOutcome>), BlockProductionError>>;

/// The type which responsible for block production.
#[must_use = "BlockProducer does nothing unless polled"]
#[allow(missing_debug_implementations)]
pub struct BlockProducer<EF: ExecutorFactory> {
    /// The inner mode of mining.
    pub producer: Arc<RwLock<BlockProducerMode<EF>>>,
}

impl<EF: ExecutorFactory> BlockProducer<EF> {
    /// Creates a block producer that mines a new block every `interval` milliseconds.
    pub fn interval(backend: Arc<Backend<EF>>, interval: u64) -> Self {
        let producer = IntervalBlockProducer::new(backend, Some(interval));
        let producer = Arc::new(RwLock::new(BlockProducerMode::Interval(producer)));
        Self { producer }
    }

    /// Creates a new block producer that will only be possible to mine by calling the
    /// `katana_generateBlock` RPC method.
    pub fn on_demand(backend: Arc<Backend<EF>>) -> Self {
        let producer = IntervalBlockProducer::new(backend, None);
        let producer = Arc::new(RwLock::new(BlockProducerMode::Interval(producer)));
        Self { producer }
    }

    /// Creates a block producer that mines a new block as soon as there are ready transactions in
    /// the transactions pool.
    pub fn instant(backend: Arc<Backend<EF>>) -> Self {
        let producer = InstantBlockProducer::new(backend);
        let producer = Arc::new(RwLock::new(BlockProducerMode::Instant(producer)));
        Self { producer }
    }

    pub(super) fn queue(&self, transactions: Vec<ExecutableTxWithHash>) {
        let mut mode = self.producer.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.queued.push_back(transactions),
            BlockProducerMode::Interval(producer) => producer.queued.push_back(transactions),
        }
    }

    pub fn validator(&self) -> TxValidator {
        let mode = self.producer.read();
        match &*mode {
            BlockProducerMode::Instant(pd) => pd.validator.clone(),
            BlockProducerMode::Interval(pd) => pd.validator.clone(),
        }
    }

    /// Returns `true` if the block producer is running in _interval_ mode. Otherwise, `fales`.
    pub fn is_interval_mining(&self) -> bool {
        matches!(*self.producer.read(), BlockProducerMode::Interval(_))
    }

    /// Returns `true` if the block producer is running in _instant_ mode. Otherwise, `fales`.
    pub fn is_instant_mining(&self) -> bool {
        matches!(*self.producer.read(), BlockProducerMode::Instant(_))
    }

    // Handler for the `katana_generateBlock` RPC method.
    pub fn force_mine(&self) {
        trace!(target: LOG_TARGET, "Scheduling force block mining.");
        let mut mode = self.producer.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.force_mine(),
            BlockProducerMode::Interval(producer) => producer.force_mine(),
        }
    }

    pub(super) fn poll_next(&self, cx: &mut Context<'_>) -> Poll<Option<BlockProductionResult>> {
        let mut mode = self.producer.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.poll_next_unpin(cx),
            BlockProducerMode::Interval(producer) => producer.poll_next_unpin(cx),
        }
    }
}

impl<EF: ExecutorFactory> Clone for BlockProducer<EF> {
    fn clone(&self) -> Self {
        BlockProducer { producer: self.producer.clone() }
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

#[allow(missing_debug_implementations)]
pub enum BlockProducerMode<EF: ExecutorFactory> {
    Interval(IntervalBlockProducer<EF>),
    Instant(InstantBlockProducer<EF>),
}

#[derive(Debug, Clone, derive_more::Deref)]
pub struct PendingExecutor(#[deref] Arc<RwLock<Box<dyn BlockExecutor<'static>>>>);

impl PendingExecutor {
    fn new(executor: Box<dyn BlockExecutor<'static>>) -> Self {
        Self(Arc::new(RwLock::new(executor)))
    }
}

#[allow(missing_debug_implementations)]
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
    tx_execution_listeners: RwLock<Vec<Sender<Vec<TxWithOutcome>>>>,

    permit: Arc<Mutex<()>>,

    /// validator used in the tx pool
    // the validator needs to always be built against the state of the block producer, so
    // im putting here for now until we find a better way to handle this.
    validator: TxValidator,
}

impl<EF: ExecutorFactory> IntervalBlockProducer<EF> {
    pub fn new(backend: Arc<Backend<EF>>, interval: Option<u64>) -> Self {
        let interval = interval.map(|time| {
            let duration = Duration::from_millis(time);
            let mut interval = interval_at(Instant::now() + duration, duration);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval
        });

        let provider = backend.blockchain.provider();

        let latest_num = provider.latest_number().unwrap();
        let mut block_env = provider.block_env_at(latest_num.into()).unwrap().unwrap();
        backend.update_block_env(&mut block_env);

        let state = provider.latest().unwrap();
        let executor = backend.executor_factory.with_state_and_block_env(state, block_env.clone());

        let permit = Arc::new(Mutex::new(()));

        // -- build the validator using the same state and envs as the executor
        let state = executor.state();
        let cfg = backend.executor_factory.cfg();
        let flags = backend.executor_factory.execution_flags();
        let validator =
            TxValidator::new(state, flags.clone(), cfg.clone(), block_env, permit.clone());

        Self {
            validator,
            permit,
            backend,
            interval,
            ongoing_mining: None,
            ongoing_execution: None,
            queued: VecDeque::default(),
            executor: PendingExecutor::new(executor),
            tx_execution_listeners: RwLock::new(vec![]),
            blocking_task_spawner: BlockingTaskPool::new().unwrap(),
        }
    }

    /// Creates a new [IntervalBlockProducer] with no `interval`. This mode will not produce blocks
    /// for every fixed interval, although it will still execute all queued transactions and
    /// keep hold of the pending state.
    pub fn new_no_mining(backend: Arc<Backend<EF>>) -> Self {
        Self::new(backend, None)
    }

    pub fn executor(&self) -> PendingExecutor {
        self.executor.clone()
    }

    /// Force mine a new block. It will only able to mine if there is no ongoing mining process.
    pub fn force_mine(&mut self) {
        match Self::do_mine(self.permit.clone(), self.executor.clone(), self.backend.clone()) {
            Ok(outcome) => {
                info!(target: LOG_TARGET, block_number = %outcome.block_number, "Force mined block.");
                self.executor =
                    self.create_new_executor_for_next_block().expect("fail to create executor");

                // update pool validator state here ---------

                let provider = self.backend.blockchain.provider();
                let state = self.executor.0.read().state();
                let num = provider.latest_number().unwrap();
                let block_env = provider.block_env_at(num.into()).unwrap().unwrap();

                self.validator.update(state, block_env);

                // -------------------------------------------

                unsafe { self.permit.raw().unlock() };
            }
            Err(e) => {
                error!(target: LOG_TARGET, error = %e, "On force mine.");
            }
        }
    }

    fn do_mine(
        permit: Arc<Mutex<()>>,
        executor: PendingExecutor,
        backend: Arc<Backend<EF>>,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        unsafe { permit.raw() }.lock();
        let executor = &mut executor.write();

        trace!(target: LOG_TARGET, "Creating new block.");

        let block_env = executor.block_env();
        let execution_output = executor.take_execution_output()?;
        let outcome = backend.do_mine_block(&block_env, execution_output)?;

        trace!(target: LOG_TARGET, block_number = %outcome.block_number, "Created new block.");

        Ok(outcome)
    }

    fn execute_transactions(
        executor: PendingExecutor,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> TxExecutionResult {
        let executor = &mut executor.write();

        let new_txs_count = transactions.len();
        executor.execute_transactions(transactions)?;

        let txs = executor.transactions();
        let total_txs = txs.len();

        // Take only the results of the newly executed transactions
        let results = txs
            .iter()
            .skip(total_txs - new_txs_count)
            .filter_map(|(tx, res)| match res {
                ExecutionResult::Failed { .. } => None,
                ExecutionResult::Success { receipt, trace, .. } => Some(TxWithOutcome {
                    tx: tx.clone(),
                    receipt: receipt.clone(),
                    exec_info: trace.clone(),
                }),
            })
            .collect::<Vec<TxWithOutcome>>();

        Ok(results)
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

    pub fn add_listener(&self) -> Receiver<Vec<TxWithOutcome>> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.tx_execution_listeners.write().push(tx);
        rx
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, txs: Vec<TxWithOutcome>) {
        let mut listener = self.tx_execution_listeners.write();
        // this is basically a retain but with mut reference
        for n in (0..listener.len()).rev() {
            let mut listener_tx = listener.swap_remove(n);
            let retain = match listener_tx.try_send(txs.clone()) {
                Ok(()) => true,
                Err(e) => {
                    if e.is_full() {
                        warn!(
                            target: LOG_TARGET,
                            "Unable to send new txs notification because channel is full.",
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
    type Item = Result<MinedBlockOutcome, BlockProductionError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if let Some(interval) = &mut pin.interval {
            // mine block if the interval is over
            if interval.poll_tick(cx).is_ready() && pin.ongoing_mining.is_none() {
                pin.ongoing_mining = Some(Box::pin({
                    let executor = pin.executor.clone();
                    let backend = pin.backend.clone();
                    let permit = pin.permit.clone();

                    pin.blocking_task_spawner.spawn(|| Self::do_mine(permit, executor, backend))
                }));
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
                                // update pool validator state here ---------

                                let provider = pin.backend.blockchain.provider();
                                let state = executor.0.read().state();
                                let num = provider.latest_number()?;
                                let block_env = provider.block_env_at(num.into()).unwrap().unwrap();

                                pin.validator.update(state, block_env);

                                // -------------------------------------------

                                pin.executor = executor;
                                unsafe { pin.permit.raw().unlock() };
                            }

                            Err(e) => return Poll::Ready(Some(Err(e))),
                        }

                        return Poll::Ready(Some(outcome));
                    }

                    Err(_) => {
                        return Poll::Ready(Some(Err(
                            BlockProductionError::ExecutionTaskCancelled,
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

#[allow(missing_debug_implementations)]
pub struct InstantBlockProducer<EF: ExecutorFactory> {
    /// Holds the backend if no block is being mined
    backend: Arc<Backend<EF>>,
    /// Single active future that mines a new block
    block_mining: Option<BlockProductionWithTxnsFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<ExecutableTxWithHash>>,

    blocking_task_pool: BlockingTaskPool,
    /// Listeners notified when a new executed tx is added.
    tx_execution_listeners: RwLock<Vec<Sender<Vec<TxWithOutcome>>>>,

    permit: Arc<Mutex<()>>,

    /// validator used in the tx pool
    // the validator needs to always be built against the state of the block producer, so
    // im putting here for now until we find a better way to handle this.
    validator: TxValidator,
}

impl<EF: ExecutorFactory> InstantBlockProducer<EF> {
    pub fn new(backend: Arc<Backend<EF>>) -> Self {
        let provider = backend.blockchain.provider();

        let permit = Arc::new(Mutex::new(()));

        let latest_num = provider.latest_number().expect("latest block num");
        let block_env = provider
            .block_env_at(latest_num.into())
            .expect("provider error")
            .expect("latest block env");

        let state = provider.latest().expect("latest state");
        let cfg = backend.executor_factory.cfg();
        let flags = backend.executor_factory.execution_flags();
        let validator =
            TxValidator::new(state, flags.clone(), cfg.clone(), block_env, permit.clone());

        Self {
            permit,
            backend,
            validator,
            block_mining: None,
            queued: VecDeque::default(),
            blocking_task_pool: BlockingTaskPool::new().unwrap(),
            tx_execution_listeners: RwLock::new(vec![]),
        }
    }

    pub fn force_mine(&mut self) {
        if self.block_mining.is_none() {
            let txs = std::mem::take(&mut self.queued);
            let _ = Self::do_mine(
                self.validator.clone(),
                self.permit.clone(),
                self.backend.clone(),
                txs,
            );
        } else {
            trace!(target: LOG_TARGET, "Unable to force mine while a mining process is running.")
        }
    }

    fn do_mine(
        validator: TxValidator,
        permit: Arc<Mutex<()>>,
        backend: Arc<Backend<EF>>,
        transactions: VecDeque<Vec<ExecutableTxWithHash>>,
    ) -> Result<(MinedBlockOutcome, Vec<TxWithOutcome>), BlockProductionError> {
        let _permit = permit.lock();

        trace!(target: LOG_TARGET, "Creating new block.");

        let transactions = transactions.into_iter().flatten().collect::<Vec<_>>();

        let provider = backend.blockchain.provider();

        // TODO: don't use the previous block env, we should create on based on the current state of
        // the l1 (to determine the proper gas prices)
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
                protocol_version: CURRENT_STARKNET_VERSION,
                sequencer_address: block_env.sequencer_address,
                l1_da_mode: L1DataAvailabilityMode::Calldata,
                l1_gas_prices: block_env.l1_gas_prices.clone(),
                l1_data_gas_prices: block_env.l1_data_gas_prices.clone(),
            },
        };

        executor.execute_block(block)?;

        let execution_output = executor.take_execution_output()?;
        let txs_outcomes = execution_output
            .transactions
            .clone()
            .into_iter()
            .filter_map(|(tx, res)| match res {
                ExecutionResult::Success { receipt, trace, .. } => {
                    Some(TxWithOutcome { tx, receipt, exec_info: trace })
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let outcome = backend.do_mine_block(&block_env, execution_output)?;

        // update pool validator state here ---------

        let provider = backend.blockchain.provider();
        let state = provider.latest()?;
        let latest_num = provider.latest_number()?;
        let block_env = provider.block_env_at(latest_num.into())?.expect("latest");
        validator.update(state, block_env);

        // -------------------------------------------

        trace!(target: LOG_TARGET, block_number = %outcome.block_number, "Created new block.");

        Ok((outcome, txs_outcomes))
    }

    pub fn add_listener(&self) -> Receiver<Vec<TxWithOutcome>> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.tx_execution_listeners.write().push(tx);
        rx
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, txs: Vec<TxWithOutcome>) {
        let mut listener = self.tx_execution_listeners.write();
        // this is basically a retain but with mut reference
        for n in (0..listener.len()).rev() {
            let mut listener_tx = listener.swap_remove(n);
            let retain = match listener_tx.try_send(txs.clone()) {
                Ok(()) => true,
                Err(e) => {
                    if e.is_full() {
                        warn!(
                            target: LOG_TARGET,
                            "Unable to send new txs notification because channel is full.",
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
            pin.block_mining = Some(Box::pin({
                // take everything that is already in the queue
                let transactions = std::mem::take(&mut pin.queued);
                let validator = pin.validator.clone();
                let backend = pin.backend.clone();
                let permit = pin.permit.clone();

                pin.blocking_task_pool
                    .spawn(|| Self::do_mine(validator, permit, backend, transactions))
            }));
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
