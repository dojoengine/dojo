use std::collections::HashMap;
use std::sync::Arc;

use blockifier::block_context::BlockContext;
use blockifier::execution::contract_class::ContractClass;
use blockifier::execution::entry_point::CallInfo;
use blockifier::state::cached_state::CommitmentStateDiff;
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{ResourcesMapping, TransactionExecutionInfo};
use blockifier::transaction::transaction_execution::Transaction as ExecutionTransaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use convert_case::{Case, Casing};
use parking_lot::RwLock;
use starknet::core::types::{Event, ExecutionResult, FieldElement, FlattenedSierraClass, MsgToL1};
use starknet_api::core::ClassHash;
use tracing::{trace, warn};

use crate::backend::storage::transaction::{
    DeclareTransaction, RejectedTransaction, Transaction, TransactionOutput,
};
use crate::db::cached::CachedStateWrapper;
use crate::db::{Database, StateExt, StateRefDb};
use crate::utils::transaction::warn_message_transaction_error_exec_error;

/// The outcome that after executing a list of transactions.
pub struct ExecutionOutcome {
    // states
    pub state_diff: CommitmentStateDiff,
    pub declared_classes: HashMap<ClassHash, ContractClass>,
    pub declared_sierra_classes: HashMap<ClassHash, FlattenedSierraClass>,
    // transactions
    pub transactions: Vec<MaybeInvalidExecutedTransaction>,
}

impl ExecutionOutcome {
    /// Apply the execution outcome to the given database.
    pub fn apply_to(&self, db: &mut dyn Database) {
        let ExecutionOutcome { state_diff, declared_classes, declared_sierra_classes, .. } = self;

        // update contract storages
        state_diff.storage_updates.iter().for_each(|(contract_address, storages)| {
            storages.iter().for_each(|(key, value)| {
                db.set_storage_at(*contract_address, *key, *value);
            })
        });

        // update declared contracts
        // apply newly declared classses
        for (class_hash, compiled_class_hash) in &state_diff.class_hash_to_compiled_class_hash {
            let contract_class =
                declared_classes.get(class_hash).expect("contract class should exist").clone();

            let is_sierra = matches!(contract_class, ContractClass::V1(_));

            db.set_contract_class(class_hash, contract_class).unwrap();
            db.set_compiled_class_hash(*class_hash, *compiled_class_hash).unwrap();

            if is_sierra {
                if let Some(class) = declared_sierra_classes.get(class_hash).cloned() {
                    db.set_sierra_class(*class_hash, class).unwrap();
                } else {
                    panic!("sierra class definition is missing")
                }
            }
        }

        // update deployed contracts
        state_diff.address_to_class_hash.iter().for_each(|(contract_address, class_hash)| {
            db.set_class_hash_at(*contract_address, *class_hash).unwrap()
        });

        // update accounts nonce
        state_diff.address_to_nonce.iter().for_each(|(contract_address, nonce)| {
            db.set_nonce(*contract_address, *nonce);
        });
    }
}

impl Default for ExecutionOutcome {
    fn default() -> Self {
        let state_diff = CommitmentStateDiff {
            storage_updates: Default::default(),
            address_to_nonce: Default::default(),
            address_to_class_hash: Default::default(),
            class_hash_to_compiled_class_hash: Default::default(),
        };

        Self {
            state_diff,
            transactions: Default::default(),
            declared_classes: Default::default(),
            declared_sierra_classes: Default::default(),
        }
    }
}

/// The result of a transaction execution.
pub type TxExecutionResult = Result<TransactionExecutionInfo, TransactionExecutionError>;

/// A transaction executor.
///
/// The transactions will be executed in an iterator fashion, sequentially, in the
/// exact order they are provided to the executor. The execution is done within its implementation
/// of the [`Iterator`] trait.
pub struct TransactionExecutor<'a> {
    /// A flag to enable/disable fee charging.
    charge_fee: bool,
    /// The block context the transactions will be executed on.
    block_context: &'a BlockContext,
    /// The transactions to be executed (in the exact order they are in the iterator).
    transactions: std::vec::IntoIter<Transaction>,
    /// The state the transactions will be executed on.
    state: &'a mut CachedStateWrapper<StateRefDb>,

    // logs flags
    error_log: bool,
    events_log: bool,
    resources_log: bool,
}

impl<'a> TransactionExecutor<'a> {
    pub fn new(
        state: &'a mut CachedStateWrapper<StateRefDb>,
        block_context: &'a BlockContext,
        charge_fee: bool,
        transactions: Vec<Transaction>,
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

impl<'a> Iterator for TransactionExecutor<'a> {
    type Item = TxExecutionResult;
    fn next(&mut self) -> Option<Self::Item> {
        self.transactions.next().map(|tx| {
            let sierra = if let Transaction::Declare(DeclareTransaction {
                sierra_class: Some(sierra_class),
                inner,
                ..
            }) = &tx
            {
                Some((inner.class_hash(), sierra_class.clone()))
            } else {
                None
            };

            let res = match tx.into() {
                ExecutionTransaction::AccountTransaction(tx) => {
                    tx.execute(&mut self.state.inner_mut(), self.block_context, self.charge_fee)
                }
                ExecutionTransaction::L1HandlerTransaction(tx) => {
                    tx.execute(&mut self.state.inner_mut(), self.block_context, self.charge_fee)
                }
            };

            match res {
                Ok(exec_info) => {
                    if let Some((class_hash, sierra_class)) = sierra {
                        self.state
                            .set_sierra_class(class_hash, sierra_class)
                            .expect("failed to set sierra class");
                    }

                    if self.error_log {
                        if let Some(err) = &exec_info.revert_error {
                            let formatted_err = format!("{:?}", err).replace("\\n", "\n");
                            warn!(target: "executor", "Transaction execution error: {formatted_err}");
                        }
                    }

                    if self.resources_log {
                        trace!(
                            target: "executor",
                            "Transaction resource usage: {}",
                            pretty_print_resources(&exec_info.actual_resources)
                        );
                    }

                    if self.events_log {
                        trace_events(&events_from_exec_info(&exec_info));
                    }

                    Ok(exec_info)
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

/// An enum which represents a transaction that has been executed and may or may not be valid.
#[derive(Clone)]
pub enum MaybeInvalidExecutedTransaction {
    Valid(Arc<ExecutedTransaction>),
    Invalid(Arc<RejectedTransaction>),
}

pub struct PendingState {
    pub state: RwLock<CachedStateWrapper<StateRefDb>>,
    /// The transactions that have been executed.
    pub executed_transactions: RwLock<Vec<MaybeInvalidExecutedTransaction>>,
}

#[derive(Debug)]
pub struct ExecutedTransaction {
    pub inner: Transaction,
    pub output: TransactionOutput,
    pub execution_info: TransactionExecutionInfo,
}

impl ExecutedTransaction {
    pub fn new(transaction: Transaction, execution_info: TransactionExecutionInfo) -> Self {
        let actual_fee = execution_info.actual_fee.0;
        let events = events_from_exec_info(&execution_info);
        let messages_sent = l2_to_l1_messages_from_exec_info(&execution_info);

        Self {
            execution_info,
            inner: transaction,
            output: TransactionOutput { actual_fee, events, messages_sent },
        }
    }

    pub fn execution_result(&self) -> ExecutionResult {
        if let Some(ref revert_err) = self.execution_info.revert_error {
            ExecutionResult::Reverted { reason: revert_err.clone() }
        } else {
            ExecutionResult::Succeeded
        }
    }
}

pub fn events_from_exec_info(execution_info: &TransactionExecutionInfo) -> Vec<Event> {
    let mut events: Vec<Event> = vec![];

    fn get_events_recursively(call_info: &CallInfo) -> Vec<Event> {
        let mut events: Vec<Event> = vec![];

        events.extend(call_info.execution.events.iter().map(|e| Event {
            from_address: (*call_info.call.storage_address.0.key()).into(),
            data: e.event.data.0.iter().map(|d| (*d).into()).collect(),
            keys: e.event.keys.iter().map(|k| k.0.into()).collect(),
        }));

        call_info.inner_calls.iter().for_each(|call| {
            events.extend(get_events_recursively(call));
        });

        events
    }

    if let Some(ref call) = execution_info.validate_call_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = execution_info.execute_call_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = execution_info.fee_transfer_call_info {
        events.extend(get_events_recursively(call));
    }

    events
}

pub fn l2_to_l1_messages_from_exec_info(execution_info: &TransactionExecutionInfo) -> Vec<MsgToL1> {
    let mut messages = vec![];

    fn get_messages_recursively(info: &CallInfo) -> Vec<MsgToL1> {
        let mut messages = vec![];

        messages.extend(info.execution.l2_to_l1_messages.iter().map(|m| MsgToL1 {
            to_address:
                FieldElement::from_byte_slice_be(m.message.to_address.0.as_bytes()).unwrap(),
            from_address: (*info.call.caller_address.0.key()).into(),
            payload: m.message.payload.0.iter().map(|p| (*p).into()).collect(),
        }));

        info.inner_calls.iter().for_each(|call| {
            messages.extend(get_messages_recursively(call));
        });

        messages
    }

    if let Some(ref info) = execution_info.validate_call_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = execution_info.execute_call_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = execution_info.fee_transfer_call_info {
        messages.extend(get_messages_recursively(info));
    }

    messages
}

fn pretty_print_resources(resources: &ResourcesMapping) -> String {
    let mut mapped_strings: Vec<_> = resources
        .0
        .iter()
        .filter_map(|(k, v)| match k.as_str() {
            "l1_gas_usage" => Some(format!("L1 Gas: {}", v)),
            "range_check_builtin" => Some(format!("Range Checks: {}", v)),
            "ecdsa_builtin" => Some(format!("ECDSA: {}", v)),
            "n_steps" => None,
            "pedersen_builtin" => Some(format!("Pedersen: {}", v)),
            "bitwise_builtin" => Some(format!("Bitwise: {}", v)),
            "keccak_builtin" => Some(format!("Keccak: {}", v)),
            _ => Some(format!("{}: {}", k.to_case(Case::Title), v)),
        })
        .collect::<Vec<String>>();

    // Sort the strings alphabetically
    mapped_strings.sort();

    // Prepend "Steps" if it exists, so it is always first
    if let Some(steps) = resources.0.get("n_steps") {
        mapped_strings.insert(0, format!("Steps: {}", steps));
    }

    mapped_strings.join(" | ")
}

fn trace_events(events: &[Event]) {
    for e in events {
        let formatted_keys =
            e.keys.iter().map(|k| format!("{k:#x}")).collect::<Vec<_>>().join(", ");

        trace!(target: "executor", "Event emitted keys=[{}]", formatted_keys);
    }
}

pub fn create_execution_outcome(
    state: &mut CachedStateWrapper<StateRefDb>,
    transactions: Vec<(Transaction, Result<TransactionExecutionInfo, TransactionExecutionError>)>,
) -> ExecutionOutcome {
    let transactions = transactions
        .into_iter()
        .map(|(tx, res)| match res {
            Ok(exec_info) => MaybeInvalidExecutedTransaction::Valid(Arc::new(
                ExecutedTransaction::new(tx, exec_info),
            )),

            Err(err) => MaybeInvalidExecutedTransaction::Invalid(Arc::new(RejectedTransaction {
                inner: tx,
                execution_error: err.to_string(),
            })),
        })
        .collect::<Vec<_>>();

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
        transactions,
        declared_classes,
        declared_sierra_classes: state.sierra_class().clone(),
    }
}
