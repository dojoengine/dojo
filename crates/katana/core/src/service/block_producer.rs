use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::{Stream, StreamExt};
use futures::FutureExt;
use katana_executor::blockifier::outcome::TxReceiptWithExecInfo;
use katana_executor::blockifier::state::{CachedStateWrapper, StateRefDb};
use katana_executor::blockifier::utils::{
    block_context_from_envs, get_state_update_from_cached_state,
};
use katana_executor::blockifier::{PendingState, TransactionExecutor};
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::receipt::Receipt;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::{ExecutableTxWithHash, TxWithHash};
use katana_provider::error::ProviderError;
use katana_provider::traits::block::BlockNumberProvider;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::StateFactoryProvider;
use parking_lot::RwLock;
use tokio::time::{interval_at, Instant, Interval};
use tracing::{trace, warn};

use crate::backend::Backend;

#[derive(Debug, thiserror::Error)]
pub enum BlockProductionError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

pub struct MinedBlockOutcome {
    pub block_number: u64,
}

type ServiceFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync>>;

type BlockProductionResult = Result<MinedBlockOutcome, BlockProductionError>;
type BlockProductionFuture = ServiceFuture<BlockProductionResult>;
type BlockProductionWithTxnsFuture =
    ServiceFuture<Result<(Vec<TxWithHashAndReceiptPair>, MinedBlockOutcome), BlockProductionError>>;
pub type TxWithHashAndReceiptPair = (TxWithHash, Receipt);

/// The type which responsible for block production.
#[must_use = "BlockProducer does nothing unless polled"]
#[derive(Clone)]
pub struct BlockProducer {
    /// The inner mode of mining.
    pub inner: Arc<RwLock<BlockProducerMode>>,
}

impl BlockProducer {
    /// Creates a block producer that mines a new block every `interval` milliseconds.
    pub fn interval(
        backend: Arc<Backend>,
        initial_state: StateRefDb,
        interval: u64,
        block_exec_envs: (BlockEnv, CfgEnv),
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockProducerMode::Interval(IntervalBlockProducer::new(
                backend,
                initial_state,
                interval,
                block_exec_envs,
            )))),
        }
    }

    /// Creates a new block producer that will only be possible to mine by calling the
    /// `katana_generateBlock` RPC method.
    pub fn on_demand(
        backend: Arc<Backend>,
        initial_state: StateRefDb,
        block_exec_envs: (BlockEnv, CfgEnv),
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockProducerMode::Interval(
                IntervalBlockProducer::new_no_mining(backend, initial_state, block_exec_envs),
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
        trace!(target: "miner", "force mining");
        let mut mode = self.inner.write();
        match &mut *mode {
            BlockProducerMode::Instant(producer) => producer.force_mine(),
            BlockProducerMode::Interval(producer) => producer.force_mine(),
        }
    }
}

impl Stream for BlockProducer {
    type Item = BlockProductionResult;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
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
pub enum BlockProducerMode {
    Interval(IntervalBlockProducer),
    Instant(InstantBlockProducer),
}

pub struct IntervalBlockProducer {
    /// The interval at which new blocks are mined.
    interval: Option<Interval>,
    backend: Arc<Backend>,
    /// Single active future that mines a new block
    block_mining: Option<BlockProductionFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<ExecutableTxWithHash>>,
    /// The state of the pending block after executing all the transactions within the interval.
    state: Arc<PendingState>,
    /// Listeners notified when a new executed tx is added.
    tx_execution_listeners: RwLock<Vec<Sender<Vec<TxWithHashAndReceiptPair>>>>,
}

impl IntervalBlockProducer {
    pub fn new(
        backend: Arc<Backend>,
        db: StateRefDb,
        interval: u64,
        block_exec_envs: (BlockEnv, CfgEnv),
    ) -> Self {
        let interval = {
            let duration = Duration::from_millis(interval);
            let mut interval = interval_at(Instant::now() + duration, duration);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval
        };

        let state = Arc::new(PendingState::new(db, block_exec_envs.0, block_exec_envs.1));

        Self {
            backend,
            state,
            block_mining: None,
            interval: Some(interval),
            queued: VecDeque::default(),
            tx_execution_listeners: RwLock::new(vec![]),
        }
    }

    /// Creates a new [IntervalBlockProducer] with no `interval`. This mode will not produce blocks
    /// for every fixed interval, although it will still execute all queued transactions and
    /// keep hold of the pending state.
    pub fn new_no_mining(
        backend: Arc<Backend>,
        db: StateRefDb,
        block_exec_envs: (BlockEnv, CfgEnv),
    ) -> Self {
        let state = Arc::new(PendingState::new(db, block_exec_envs.0, block_exec_envs.1));

        Self {
            state,
            backend,
            interval: None,
            block_mining: None,
            queued: VecDeque::default(),
            tx_execution_listeners: RwLock::new(vec![]),
        }
    }

    pub fn state(&self) -> Arc<PendingState> {
        self.state.clone()
    }

    /// Force mine a new block. It will only able to mine if there is no ongoing mining process.
    pub fn force_mine(&self) {
        if self.block_mining.is_none() {
            let outcome = self.outcome();
            let _ = Self::do_mine(outcome, self.backend.clone(), self.state.clone());
        } else {
            trace!(target: "miner", "unable to force mine while a mining process is running")
        }
    }

    fn do_mine(
        state_updates: StateUpdatesWithDeclaredClasses,
        backend: Arc<Backend>,
        pending_state: Arc<PendingState>,
    ) -> BlockProductionResult {
        trace!(target: "miner", "creating new block");

        let (txs, _) = pending_state.take_txs_all();
        let tx_receipt_pairs =
            txs.into_iter().map(|(tx, rct)| (tx, rct.receipt)).collect::<Vec<_>>();

        let (mut block_env, cfg_env) = pending_state.block_execution_envs();

        let (outcome, new_state) =
            backend.mine_pending_block(&block_env, tx_receipt_pairs, state_updates)?;

        trace!(target: "miner", "created new block: {}", outcome.block_number);

        backend.update_block_env(&mut block_env);
        pending_state.reset_state(StateRefDb(new_state), block_env, cfg_env);

        Ok(outcome)
    }

    fn execute_transactions(&self, transactions: Vec<ExecutableTxWithHash>) {
        let txs = transactions.iter().map(TxWithHash::from);

        let block_context = block_context_from_envs(
            &self.state.block_envs.read().0,
            &self.state.block_envs.read().1,
        );

        let results = {
            TransactionExecutor::new(
                &self.state.state,
                &block_context,
                !self.backend.config.disable_fee,
                !self.backend.config.disable_validate,
                transactions.clone().into_iter(),
            )
            .with_error_log()
            .with_events_log()
            .with_resources_log()
            .zip(txs)
            .filter_map(|(res, tx)| {
                let Ok(info) = res else { return None };
                let receipt = TxReceiptWithExecInfo::new(&tx, info);
                Some((tx, receipt))
            })
            .collect::<Vec<_>>()
        };

        self.state.executed_txs.write().extend(results.clone());
        self.notify_listener(results.into_iter().map(|(tx, info)| (tx, info.receipt)).collect());
    }

    pub fn add_listener(&self) -> Receiver<Vec<TxWithHashAndReceiptPair>> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.tx_execution_listeners.write().push(tx);
        rx
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, txs: Vec<TxWithHashAndReceiptPair>) {
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

    fn outcome(&self) -> StateUpdatesWithDeclaredClasses {
        get_state_update_from_cached_state(&self.state.state)
    }
}

impl Stream for IntervalBlockProducer {
    // mined block outcome and the new state
    type Item = BlockProductionResult;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if let Some(interval) = &mut pin.interval {
            if interval.poll_tick(cx).is_ready() && pin.block_mining.is_none() {
                let backend = pin.backend.clone();
                let outcome = pin.outcome();
                let state = pin.state.clone();

                pin.block_mining = Some(Box::pin(async move {
                    tokio::task::spawn_blocking(|| Self::do_mine(outcome, backend, state))
                        .await
                        .unwrap()
                }));
            }
        }

        // only execute transactions if there is no mining in progress
        if !pin.queued.is_empty() && pin.block_mining.is_none() {
            let transactions = pin.queued.pop_front().expect("not empty; qed");
            pin.execute_transactions(transactions);
        }

        // poll the mining future
        if let Some(mut mining) = pin.block_mining.take() {
            // reset the executor for the next block
            if let Poll::Ready(outcome) = mining.poll_unpin(cx) {
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
    block_mining: Option<BlockProductionWithTxnsFuture>,
    /// Backlog of sets of transactions ready to be mined
    queued: VecDeque<Vec<ExecutableTxWithHash>>,
    /// Listeners notified when a new executed tx is added.
    tx_execution_listeners: RwLock<Vec<Sender<Vec<TxWithHashAndReceiptPair>>>>,
}

impl InstantBlockProducer {
    pub fn new(backend: Arc<Backend>) -> Self {
        Self {
            backend,
            block_mining: None,
            queued: VecDeque::default(),
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
        backend: Arc<Backend>,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> Result<(Vec<TxWithHashAndReceiptPair>, MinedBlockOutcome), BlockProductionError> {
        trace!(target: "miner", "creating new block");

        let provider = backend.blockchain.provider();

        let cfg_env = backend.chain_cfg_env();
        let latest_num = provider.latest_number()?;
        let mut block_env = provider.block_env_at(BlockHashOrNumber::Num(latest_num))?.unwrap();
        backend.update_block_env(&mut block_env);

        let block_context = block_context_from_envs(&block_env, &cfg_env);

        let latest_state = StateFactoryProvider::latest(backend.blockchain.provider())?;
        let state = CachedStateWrapper::new(StateRefDb(latest_state));

        let txs = transactions.iter().map(TxWithHash::from);

        let tx_receipt_pairs: Vec<TxWithHashAndReceiptPair> = TransactionExecutor::new(
            &state,
            &block_context,
            !backend.config.disable_fee,
            !backend.config.disable_validate,
            transactions.clone().into_iter(),
        )
        .with_error_log()
        .with_events_log()
        .with_resources_log()
        .zip(txs)
        .filter_map(|(res, tx)| {
            if let Ok(info) = res {
                let info = TxReceiptWithExecInfo::new(&tx, info);
                Some((tx, info.receipt))
            } else {
                None
            }
        })
        .collect();

        let outcome = backend.do_mine_block(
            &block_env,
            tx_receipt_pairs.clone(),
            get_state_update_from_cached_state(&state),
        )?;

        trace!(target: "miner", "created new block: {}", outcome.block_number);

        Ok((tx_receipt_pairs, outcome))
    }

    pub fn add_listener(&self) -> Receiver<Vec<TxWithHashAndReceiptPair>> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.tx_execution_listeners.write().push(tx);
        rx
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, txs: Vec<TxWithHashAndReceiptPair>) {
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

impl Stream for InstantBlockProducer {
    // mined block outcome and the new state
    type Item = Result<MinedBlockOutcome, BlockProductionError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if !pin.queued.is_empty() && pin.block_mining.is_none() {
            let transactions = pin.queued.pop_front().expect("not empty; qed");
            let backend = pin.backend.clone();

            pin.block_mining = Some(Box::pin(async move {
                tokio::task::spawn_blocking(|| Self::do_mine(backend, transactions)).await.unwrap()
            }));
        }

        // poll the mining future
        if let Some(mut mining) = pin.block_mining.take() {
            match mining.poll_unpin(cx) {
                Poll::Ready(Ok((txs, outcome))) => {
                    pin.notify_listener(txs);
                    return Poll::Ready(Some(Ok(outcome)));
                }

                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Some(Err(e)));
                }

                Poll::Pending => {
                    pin.block_mining = Some(mining);
                }
            }
        }

        Poll::Pending
    }
}
