use std::sync::Arc;

use blockifier::block_context::BlockContext;
use blockifier::execution::entry_point::CallInfo;
use blockifier::state::cached_state::CachedState;
use blockifier::state::state_api::StateReader;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{ResourcesMapping, TransactionExecutionInfo};
use blockifier::transaction::transaction_execution::Transaction as ExecutionTransaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use convert_case::{Case, Casing};
use parking_lot::RwLock;
use starknet::core::types::{Event, FieldElement, MsgToL1};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{info, trace, warn};

use super::state::MemDb;
use super::storage::block::{PartialBlock, PartialHeader};
use super::storage::transaction::{RejectedTransaction, Transaction, TransactionOutput};
use super::storage::BlockchainStorage;
use crate::backend::storage::transaction::{DeclareTransaction, KnownTransaction};
use crate::env::Env;

#[derive(Debug)]
pub struct PendingBlockExecutor {
    pub parent_hash: FieldElement,
    /// The state of the pending block. It is the state that the
    /// transaction included in the pending block will be executed on.
    /// The changes made after the execution of a transaction will be
    /// persisted for the next included transaction.
    pub state: CachedState<MemDb>,
    pub storage: Arc<AsyncRwLock<BlockchainStorage>>,
    pub env: Arc<RwLock<Env>>,
    pub transactions: Vec<Arc<ExecutedTransaction>>,
    pub outputs: Vec<TransactionOutput>,
}

impl PendingBlockExecutor {
    pub fn new(
        parent_hash: FieldElement,
        state: MemDb,
        env: Arc<RwLock<Env>>,
        storage: Arc<AsyncRwLock<BlockchainStorage>>,
    ) -> Self {
        Self {
            env,
            storage,
            parent_hash,
            outputs: Vec::new(),
            transactions: Vec::new(),
            state: CachedState::new(state),
        }
    }

    pub fn as_block(&self) -> PartialBlock {
        let block_context = &self.env.read().block;

        let header = PartialHeader {
            parent_hash: self.parent_hash,
            gas_price: block_context.gas_price,
            number: block_context.block_number.0,
            timestamp: block_context.block_timestamp.0,
            sequencer_address: (*block_context.sequencer_address.0.key()).into(),
        };

        PartialBlock {
            header,
            outputs: self.outputs.clone(),
            transactions: self.transactions.clone(),
        }
    }

    // Add a transaction to the executor. The transaction will be executed
    // on the pending state. The transaction will be added to the pending block
    // if it passes the validation logic. Otherwise, the transaction will be
    // rejected. On both cases, the transaction will still be stored in the
    // storage.
    pub async fn add_transaction(&mut self, transaction: Transaction, charge_fee: bool) -> bool {
        let transaction_hash = transaction.hash();

        info!("Transaction received | Hash: {transaction_hash:#x}");

        let res = execute_transaction(
            match transaction {
                Transaction::L1Handler(_) => ExecutionTransaction::L1HandlerTransaction(
                    transaction.clone().into()),
                Transaction::Declare(_) => ExecutionTransaction::AccountTransaction(
                    transaction.clone().into()),
                Transaction::DeployAccount(_) => ExecutionTransaction::AccountTransaction(
                    transaction.clone().into()),
                Transaction::Invoke(_) => ExecutionTransaction::AccountTransaction(
                    transaction.clone().into()),
            },
            &mut self.state,
            &self.env.read().block,
            charge_fee,
        );

        match res {
            Ok(execution_info) => {
                trace!(
                    "Transaction resource usage: {}",
                    pretty_print_resources(&execution_info.actual_resources)
                );

                // Because `State` trait from `blockifier` doesn't have a method to set the
                // `sierra_class` of a contract, we need to do it manually.
                if let Transaction::Declare(DeclareTransaction {
                    inner,
                    sierra_class: Some(sierra_class),
                    ..
                }) = &transaction
                {
                    let class_hash = inner.class_hash();
                    self.state.state.sierra_classes.insert(class_hash, sierra_class.clone());
                }

                let executed_tx = Arc::new(ExecutedTransaction::new(transaction, execution_info));

                trace_events(&executed_tx.output.events);

                self.outputs.push(executed_tx.output.clone());
                self.transactions.push(executed_tx);

                true
            }

            Err(err) => {
                self.storage.write().await.transactions.insert(
                    transaction_hash,
                    KnownTransaction::Rejected(Box::new(RejectedTransaction {
                        transaction: transaction.into(),
                        execution_error: err.to_string(),
                    })),
                );

                false
            }
        }
    }
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
        let events = Self::events(&execution_info);
        let messages_sent = Self::l2_to_l1_messages(&execution_info);

        Self {
            inner: transaction,
            execution_info,
            output: TransactionOutput { actual_fee, events, messages_sent },
        }
    }

    fn events(execution_info: &TransactionExecutionInfo) -> Vec<Event> {
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

    fn l2_to_l1_messages(execution_info: &TransactionExecutionInfo) -> Vec<MsgToL1> {
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
}

pub fn execute_transaction<S: StateReader>(
    transaction: ExecutionTransaction,
    pending_state: &mut CachedState<S>,
    block_context: &BlockContext,
    charge_fee: bool,
) -> Result<TransactionExecutionInfo, TransactionExecutionError> {
    let res = match transaction {
        ExecutionTransaction::AccountTransaction(tx) => {
            tx.execute(pending_state, block_context, charge_fee)
        }
        ExecutionTransaction::L1HandlerTransaction(tx) => {
            tx.execute(pending_state, block_context, charge_fee)
        }
    };

    match res {
        Ok(exec_info) => {
            if let Some(err) = &exec_info.revert_error {
                let formatted_err = format!("{:?}", err).replace("\\n", "\n");
                warn!("Transaction execution error: {formatted_err}");
            }
            Ok(exec_info)
        }
        Err(err) => {
            warn!("Transaction validation error: {err:?}");
            Err(err)
        }
    }
}

pub fn pretty_print_resources(resources: &ResourcesMapping) -> String {
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

pub fn trace_events(events: &[Event]) {
    for e in events {
        let formatted_keys =
            e.keys.iter().map(|k| format!("{k:#x}")).collect::<Vec<_>>().join(", ");

        trace!("Event emitted keys=[{}]", formatted_keys);
    }
}
