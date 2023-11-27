use std::collections::HashMap;

use blockifier::execution::entry_point::CallInfo;
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{ResourcesMapping, TransactionExecutionInfo};
use convert_case::{Case, Casing};
use katana_primitives::transaction::Tx;
use katana_primitives::FieldElement;
use starknet::core::types::{Event, MsgToL1};
use starknet::core::utils::parse_cairo_short_string;
use tracing::trace;

use super::outcome::{ExecutedTx, ExecutionOutcome};
use super::state::{CachedStateWrapper, StateRefDb};

pub(crate) fn warn_message_transaction_error_exec_error(err: &TransactionExecutionError) {
    match err {
        TransactionExecutionError::EntryPointExecutionError(ref eperr)
        | TransactionExecutionError::ExecutionError(ref eperr) => match eperr {
            EntryPointExecutionError::ExecutionFailed { error_data } => {
                let mut reasons: Vec<String> = vec![];
                error_data.iter().for_each(|felt| {
                    if let Ok(s) = parse_cairo_short_string(&FieldElement::from(*felt)) {
                        reasons.push(s);
                    }
                });

                tracing::warn!(target: "executor",
                               "Transaction validation error: {}", reasons.join(" "));
            }
            _ => tracing::warn!(target: "executor",
                                "Transaction validation error: {:?}", err),
        },
        _ => tracing::warn!(target: "executor",
                            "Transaction validation error: {:?}", err),
    }
}

pub(crate) fn pretty_print_resources(resources: &ResourcesMapping) -> String {
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

pub(crate) fn trace_events(events: &[Event]) {
    for e in events {
        let formatted_keys =
            e.keys.iter().map(|k| format!("{k:#x}")).collect::<Vec<_>>().join(", ");

        trace!(target: "executor", "Event emitted keys=[{}]", formatted_keys);
    }
}

pub fn create_execution_outcome(
    state: &mut CachedStateWrapper<StateRefDb>,
    executed_txs: Vec<(Tx, TransactionExecutionInfo)>,
) -> ExecutionOutcome {
    let transactions = executed_txs.into_iter().map(|(tx, res)| ExecutedTx::new(tx, res)).collect();
    let state_diff = state.to_state_diff();
    let declared_classes = state_diff
        .class_hash_to_compiled_class_hash
        .iter()
        .map(|(class_hash, _)| {
            let contract_class = state
                .get_compiled_contract_class(class_hash)
                .expect("qed; class must exist if declared");
            (class_hash.0.into(), contract_class)
        })
        .collect::<HashMap<FieldElement, _>>();

    ExecutionOutcome {
        state_diff,
        transactions,
        declared_classes,
        declared_sierra_classes: state.sierra_class().clone(),
    }
}

pub(crate) fn events_from_exec_info(execution_info: &TransactionExecutionInfo) -> Vec<Event> {
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

pub(crate) fn l2_to_l1_messages_from_exec_info(
    execution_info: &TransactionExecutionInfo,
) -> Vec<MsgToL1> {
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
