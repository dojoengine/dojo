pub mod outcome;
pub mod state;
pub mod utils;

use std::sync::Arc;

use blockifier::block_context::BlockContext;
use blockifier::state::state_api::StateReader;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::TransactionExecutionInfo;
use blockifier::transaction::transaction_execution::Transaction as BlockifierExecuteTx;
use blockifier::transaction::transactions::ExecutableTransaction;
use katana_primitives::transaction::{DeclareTxWithClasses, ExecutionTx};
use parking_lot::RwLock;
use tracing::{trace, warn};

use self::outcome::ExecutedTx;
use self::state::{CachedStateWrapper, StateRefDb};
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
pub struct TransactionExecutor<'a, S: StateReader> {
    /// A flag to enable/disable fee charging.
    charge_fee: bool,
    /// The block context the transactions will be executed on.
    block_context: &'a BlockContext,
    /// The transactions to be executed (in the exact order they are in the iterator).
    transactions: std::vec::IntoIter<ExecutionTx>,
    /// The state the transactions will be executed on.
    state: &'a mut CachedStateWrapper<S>,

    // logs flags
    error_log: bool,
    events_log: bool,
    resources_log: bool,
}

impl<'a, S: StateReader> TransactionExecutor<'a, S> {
    pub fn new(
        state: &'a mut CachedStateWrapper<S>,
        block_context: &'a BlockContext,
        charge_fee: bool,
        transactions: Vec<ExecutionTx>,
    ) -> Self {
        Self {
            state,
            charge_fee,
            block_context,
            error_log: false,
            events_log: false,
            resources_log: false,
            transactions: transactions.into_iter(),
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

impl<'a, S: StateReader> Iterator for TransactionExecutor<'a, S> {
    type Item = TxExecutionResult;
    fn next(&mut self) -> Option<Self::Item> {
        self.transactions.next().map(|tx| {
            let res = execute_tx(tx, &mut self.state, self.block_context, self.charge_fee);

            match res {
                Ok(info) => {
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
                        trace_events(&events_from_exec_info(&info));
                    }

                    Ok(info)
                }

                Err(err) => {
                    if self.error_log {
                        warn_message_transaction_error_exec_error(&err);
                    }

                    Err(err)
                }
            }
        })
    }
}

pub struct PendingState {
    pub state: RwLock<CachedStateWrapper<StateRefDb>>,
    /// The transactions that have been executed.
    pub executed_transactions: RwLock<Vec<Arc<ExecutedTx>>>,
}

fn execute_tx<S: StateReader>(
    tx: ExecutionTx,
    state: &mut CachedStateWrapper<S>,
    block_context: &BlockContext,
    charge_fee: bool,
) -> TxExecutionResult {
    let sierra = if let ExecutionTx::Declare(DeclareTxWithClasses {
        tx,
        sierra_class: Some(sierra_class),
        ..
    }) = &tx
    {
        Some((tx.class_hash, sierra_class.clone()))
    } else {
        None
    };

    let res = match tx.into() {
        BlockifierExecuteTx::AccountTransaction(tx) => {
            tx.execute(&mut state.inner_mut(), block_context, charge_fee)
        }
        BlockifierExecuteTx::L1HandlerTransaction(tx) => {
            tx.execute(&mut state.inner_mut(), block_context, charge_fee)
        }
    };

    if let res @ Ok(_) = res {
        if let Some((class_hash, sierra_class)) = sierra {
            state.sierra_class_mut().insert(class_hash, sierra_class);
        }

        res
    } else {
        res
    }
}
