use std::collections::HashMap;

use katana_primitives::receipt::{
    DeclareTxReceipt, DeployAccountTxReceipt, Event, InvokeTxReceipt, L1HandlerTxReceipt,
    MessageToL1, Receipt, TxExecutionResources,
};
use katana_primitives::trace::{CallInfo, TxExecInfo};
use katana_primitives::transaction::Tx;

pub(super) fn receipt_from_exec_info(tx: &Tx, info: &TxExecInfo) -> Receipt {
    let actual_fee = info.actual_fee;
    let events = events_from_exec_info(info);
    let revert_error = info.revert_error.clone();
    let messages_sent = l2_to_l1_messages_from_exec_info(info);
    let actual_resources = parse_actual_resources(&info.actual_resources);

    match tx {
        Tx::Invoke(_) => Receipt::Invoke(InvokeTxReceipt {
            events,
            actual_fee,
            revert_error,
            messages_sent,
            execution_resources: actual_resources,
        }),

        Tx::Declare(_) => Receipt::Declare(DeclareTxReceipt {
            events,
            actual_fee,
            revert_error,
            messages_sent,
            execution_resources: actual_resources,
        }),

        Tx::L1Handler(tx) => Receipt::L1Handler(L1HandlerTxReceipt {
            events,
            actual_fee,
            revert_error,
            messages_sent,
            message_hash: tx.message_hash,
            execution_resources: actual_resources,
        }),

        Tx::DeployAccount(tx) => Receipt::DeployAccount(DeployAccountTxReceipt {
            events,
            actual_fee,
            revert_error,
            messages_sent,
            execution_resources: actual_resources,
            contract_address: tx.contract_address(),
        }),
    }
}

fn events_from_exec_info(info: &TxExecInfo) -> Vec<Event> {
    let mut events: Vec<Event> = vec![];

    fn get_events_recursively(call_info: &CallInfo) -> Vec<Event> {
        let mut events: Vec<Event> = vec![];

        events.extend(call_info.events.iter().map(|e| Event {
            from_address: call_info.contract_address,
            data: e.data.clone(),
            keys: e.keys.clone(),
        }));

        call_info.inner_calls.iter().for_each(|call| {
            events.extend(get_events_recursively(call));
        });

        events
    }

    if let Some(ref call) = info.validate_call_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = info.execute_call_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = info.fee_transfer_call_info {
        events.extend(get_events_recursively(call));
    }

    events
}

fn l2_to_l1_messages_from_exec_info(info: &TxExecInfo) -> Vec<MessageToL1> {
    let mut messages = vec![];

    fn get_messages_recursively(info: &CallInfo) -> Vec<MessageToL1> {
        let mut messages = vec![];

        messages.extend(info.l2_to_l1_messages.iter().map(|m| MessageToL1 {
            from_address: m.from_address,
            payload: m.payload.clone(),
            to_address: m.to_address,
        }));

        info.inner_calls.iter().for_each(|call| {
            messages.extend(get_messages_recursively(call));
        });

        messages
    }

    if let Some(ref info) = info.validate_call_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = info.execute_call_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = info.fee_transfer_call_info {
        messages.extend(get_messages_recursively(info));
    }

    messages
}

fn parse_actual_resources(resources: &HashMap<String, u64>) -> TxExecutionResources {
    TxExecutionResources {
        steps: resources.get("n_steps").copied().unwrap_or_default(),
        memory_holes: resources.get("memory_holes").copied(),
        ec_op_builtin: resources.get("ec_op_builtin").copied(),
        ecdsa_builtin: resources.get("ecdsa_builtin").copied(),
        keccak_builtin: resources.get("keccak_builtin").copied(),
        bitwise_builtin: resources.get("bitwise_builtin").copied(),
        pedersen_builtin: resources.get("pedersen_builtin").copied(),
        poseidon_builtin: resources.get("poseidon_builtin").copied(),
        range_check_builtin: resources.get("range_check_builtin").copied(),
        segment_arena_builtin: resources.get("segment_arena_builtin").copied(),
    }
}
