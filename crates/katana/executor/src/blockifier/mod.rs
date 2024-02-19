pub mod outcome;
pub mod state;
pub mod transactions;
pub mod utils;

use std::sync::Arc;

use blockifier::block_context::BlockContext;
use blockifier::state::state_api::StateReader;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::TransactionExecutionInfo;
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxWithHash};
use katana_provider::traits::state::StateProvider;
use parking_lot::RwLock;
use tracing::{trace, warn};

use self::outcome::TxReceiptWithExecInfo;
use self::state::{CachedStateWrapper, StateRefDb};
use self::transactions::BlockifierTx;
use self::utils::events_from_exec_info;
use crate::blockifier::utils::{
    pretty_print_resources, trace_events, warn_message_transaction_error_exec_error,
};

/// The result of a transaction execution.
type TxExecutionResult = Result<TransactionExecutionInfo, TransactionExecutionError>;

/// A transaction executor.
///
/// The transactions will be executed in an iterator fashion, sequentially, in the
/// exact order they are provided to the executor. The execution is done within its
/// implementation of the [`Iterator`] trait.
pub struct TransactionExecutor<'a, S: StateReader + StateProvider, T> {
    /// A flag to enable/disable fee charging.
    charge_fee: bool,
    /// The block context the transactions will be executed on.
    block_context: &'a BlockContext,
    /// The transactions to be executed (in the exact order they are in the iterator).
    transactions: T,
    /// The state the transactions will be executed on.
    state: &'a CachedStateWrapper<S>,
    /// A flag to enable/disable transaction validation.
    validate: bool,

    // logs flags
    error_log: bool,
    events_log: bool,
    resources_log: bool,
}

impl<'a, S, T> TransactionExecutor<'a, S, T>
where
    S: StateReader + StateProvider,
    T: Iterator<Item = ExecutableTxWithHash>,
{
    pub fn new(
        state: &'a CachedStateWrapper<S>,
        block_context: &'a BlockContext,
        charge_fee: bool,
        validate: bool,
        transactions: T,
    ) -> Self {
        Self {
            state,
            charge_fee,
            transactions,
            block_context,
            validate,
            error_log: false,
            events_log: false,
            resources_log: false,
        }
    }

    pub fn with_events_log(self) -> Self {
        Self { events_log: true, ..self }
    }

    pub fn with_error_log(self) -> Self {
        Self { error_log: true, ..self }
    }

    pub fn with_resources_log(self) -> Self {
        Self { resources_log: true, ..self }
    }

    /// A method to conveniently execute all the transactions and return their results.
    pub fn execute(self) -> Vec<TxExecutionResult> {
        self.collect()
    }
}

impl<'a, S, T> Iterator for TransactionExecutor<'a, S, T>
where
    S: StateReader + StateProvider,
    T: Iterator<Item = ExecutableTxWithHash>,
{
    type Item = TxExecutionResult;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.transactions.next().map(|tx| {
            execute_tx(tx, self.state, self.block_context, self.charge_fee, self.validate)
        })?;

        match res {
            Ok(ref info) => {
                if self.error_log {
                    if let Some(err) = &info.revert_error {
                        let formatted_err = format!("{err:?}").replace("\\n", "\n");
                        warn!(target: "executor", "Transaction execution error: {formatted_err}");
                    }
                }

                if self.resources_log {
                    trace!(
                        target: "executor",
                        "Transaction resource usage: {}",
                        pretty_print_resources(&info.actual_resources)
                    );
                }

                if self.events_log {
                    trace_events(&events_from_exec_info(info));
                }

                Some(res)
            }

            Err(ref err) => {
                if self.error_log {
                    warn_message_transaction_error_exec_error(err);
                }

                Some(res)
            }
        }
    }
}

fn execute_tx<S>(
    tx: ExecutableTxWithHash,
    state: &CachedStateWrapper<S>,
    block_context: &BlockContext,
    charge_fee: bool,
    validate: bool,
) -> TxExecutionResult
where
    S: StateReader + StateProvider,
{
    let class_declaration_params = if let ExecutableTx::Declare(tx) = tx.as_ref() {
        let class_hash = tx.class_hash();
        Some((class_hash, tx.compiled_class.clone(), tx.sierra_class.clone()))
    } else {
        None
    };

    let res = match BlockifierTx::from(tx).0 {
        Transaction::AccountTransaction(tx) => {
            tx.execute(&mut state.inner(), block_context, charge_fee, validate)
        }
        Transaction::L1HandlerTransaction(tx) => {
            tx.execute(&mut state.inner(), block_context, charge_fee, validate)
        }
    };

    if res.is_ok() {
        if let Some((class_hash, compiled_class, sierra_class)) = class_declaration_params {
            state.class_cache.write().compiled.insert(class_hash, compiled_class);

            if let Some(sierra_class) = sierra_class {
                state.class_cache.write().sierra.insert(class_hash, sierra_class);
            }
        }
    }

    res
}

pub type AcceptedTxPair = (TxWithHash, TxReceiptWithExecInfo);
pub type RejectedTxPair = (TxWithHash, TransactionExecutionError);

pub struct PendingState {
    /// The block context of the pending block.
    pub block_envs: RwLock<(BlockEnv, CfgEnv)>,
    /// The state of the pending block.
    pub state: Arc<CachedStateWrapper<StateRefDb>>,
    /// The transactions that have been executed.
    pub executed_txs: RwLock<Vec<AcceptedTxPair>>,
    /// The transactions that have been rejected.
    pub rejected_txs: RwLock<Vec<RejectedTxPair>>,
}

impl PendingState {
    pub fn new(state: StateRefDb, block_env: BlockEnv, cfg_env: CfgEnv) -> Self {
        Self {
            block_envs: RwLock::new((block_env, cfg_env)),
            state: Arc::new(CachedStateWrapper::new(state)),
            executed_txs: RwLock::new(Vec::new()),
            rejected_txs: RwLock::new(Vec::new()),
        }
    }

    pub fn reset_state(&self, state: Box<dyn StateProvider>, block_env: BlockEnv, cfg_env: CfgEnv) {
        *self.block_envs.write() = (block_env, cfg_env);
        self.state.reset_with_new_state(StateRefDb(state));
    }

    pub fn add_executed_txs(&self, transactions: Vec<(TxWithHash, TxExecutionResult)>) {
        transactions.into_iter().for_each(|(tx, res)| self.add_executed_tx(tx, res));
    }

    /// Drain the pending transactions, returning the executed and rejected transactions.
    pub fn take_txs_all(&self) -> (Vec<AcceptedTxPair>, Vec<RejectedTxPair>) {
        let executed_txs = std::mem::take(&mut *self.executed_txs.write());
        let rejected_txs = std::mem::take(&mut *self.rejected_txs.write());
        (executed_txs, rejected_txs)
    }

    pub fn block_execution_envs(&self) -> (BlockEnv, CfgEnv) {
        self.block_envs.read().clone()
    }

    fn add_executed_tx(&self, tx: TxWithHash, execution_result: TxExecutionResult) {
        match execution_result {
            Ok(execution_info) => {
                let receipt = TxReceiptWithExecInfo::new(&tx, execution_info);
                self.executed_txs.write().push((tx, receipt));
            }
            Err(err) => {
                self.rejected_txs.write().push((tx, err));
            }
        }
    }
}
