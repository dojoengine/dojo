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
use katana_primitives::transaction::{
    DeclareTxWithClass, ExecutableTx, ExecutableTxWithHash, TxWithHash,
};
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
pub struct TransactionExecutor<'a, S: StateReader, T> {
    /// A flag to enable/disable fee charging.
    charge_fee: bool,
    /// The block context the transactions will be executed on.
    block_context: &'a BlockContext,
    /// The transactions to be executed (in the exact order they are in the iterator).
    transactions: T,
    /// The state the transactions will be executed on.
    state: &'a CachedStateWrapper<S>,

    // logs flags
    error_log: bool,
    events_log: bool,
    resources_log: bool,
}

impl<'a, S, T> TransactionExecutor<'a, S, T>
where
    S: StateReader,
    T: Iterator<Item = ExecutableTxWithHash>,
{
    pub fn new(
        state: &'a CachedStateWrapper<S>,
        block_context: &'a BlockContext,
        charge_fee: bool,
        transactions: T,
    ) -> Self {
        Self {
            state,
            charge_fee,
            transactions,
            block_context,
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
    S: StateReader,
    T: Iterator<Item = ExecutableTxWithHash>,
{
    type Item = TxExecutionResult;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self
            .transactions
            .next()
            .map(|tx| execute_tx(tx, self.state, self.block_context, self.charge_fee))?;

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

fn execute_tx<S: StateReader>(
    tx: ExecutableTxWithHash,
    state: &CachedStateWrapper<S>,
    block_context: &BlockContext,
    charge_fee: bool,
) -> TxExecutionResult {
    // TODO: check how this value must be controlled.
    let validate = true;

    let sierra = if let ExecutableTx::Declare(DeclareTxWithClass {
        transaction,
        sierra_class: Some(sierra_class),
        ..
    }) = tx.as_ref()
    {
        Some((transaction.class_hash(), sierra_class.clone()))
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
        if let Some((class_hash, sierra_class)) = sierra {
            state.sierra_class_mut().insert(class_hash, sierra_class);
        }
    }

    res
}

pub type AcceptedTxPair = (TxWithHash, TxReceiptWithExecInfo);
pub type RejectedTxPair = (TxWithHash, TransactionExecutionError);

pub struct PendingState {
    pub state: Arc<CachedStateWrapper<StateRefDb>>,
    /// The transactions that have been executed.
    pub executed_txs: RwLock<Vec<(TxWithHash, TxReceiptWithExecInfo)>>,
    /// The transactions that have been rejected.
    pub rejected_txs: RwLock<Vec<(TxWithHash, TransactionExecutionError)>>,
}

impl PendingState {
    pub fn new(state: StateRefDb) -> Self {
        Self {
            state: Arc::new(CachedStateWrapper::new(state)),
            executed_txs: RwLock::new(Vec::new()),
            rejected_txs: RwLock::new(Vec::new()),
        }
    }

    pub fn reset_state_with(&self, state: StateRefDb) {
        self.state.reset_with_new_state(state);
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
