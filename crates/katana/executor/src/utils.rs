use katana_primitives::fee::TxFeeInfo;
use katana_primitives::receipt::{
    DeclareTxReceipt, DeployAccountTxReceipt, Event, InvokeTxReceipt, L1HandlerTxReceipt,
    MessageToL1, Receipt,
};
use katana_primitives::trace::{CallInfo, TxExecInfo, TxResources};
use katana_primitives::transaction::TxRef;
use tracing::trace;

pub(crate) const LOG_TARGET: &str = "executor";

pub fn log_resources(resources: &TxResources, receipt: &Receipt) {
    let mut mapped_strings = Vec::new();

    for (builtin, count) in &resources.vm_resources.builtin_instance_counter {
        mapped_strings.push(format!("{builtin}: {count}"));
    }

    // Sort the strings alphabetically
    mapped_strings.sort();
    mapped_strings.insert(0, format!("steps: {}", resources.vm_resources.n_steps));
    mapped_strings.insert(1, format!("memory holes: {}", resources.vm_resources.n_memory_holes));

    trace!(target: LOG_TARGET, usage = mapped_strings.join(" | "), "Transaction resource usage.");

    let mut mapped_strings = Vec::new();
    mapped_strings.push(format!("L1: {}", resources.total_gas_consumed.l1_gas));
    mapped_strings.push(format!("L1 Data: {}", resources.total_gas_consumed.l1_data_gas));
    mapped_strings.push(format!("DA L1: {}", resources.data_availability.l1_gas));
    mapped_strings.push(format!("DA L1 Data: {}", resources.data_availability.l1_data_gas));

    mapped_strings.push(format!("Gas consumed: {}", receipt.fee().gas_consumed));
    mapped_strings.push(format!(
        "Overall Fee: {:.8} {}",
        receipt.fee().overall_fee as f64 / 100.0 / 10_f64.powf(18.0),
        match receipt.fee().unit {
            katana_primitives::fee::PriceUnit::Wei => "ETH",
            katana_primitives::fee::PriceUnit::Fri => "STRK",
        },
    ));

    trace!(target: LOG_TARGET, usage = mapped_strings.join(" | "), "Gas usage.");
}

pub(crate) fn build_receipt(tx: TxRef<'_>, fee: TxFeeInfo, info: &TxExecInfo) -> Receipt {
    let events = events_from_exec_info(info);
    let revert_error = info.revert_error.clone();
    let messages_sent = l2_to_l1_messages_from_exec_info(info);
    let actual_resources = info.actual_resources.clone();

    match tx {
        TxRef::Invoke(_) => Receipt::Invoke(InvokeTxReceipt {
            events,
            fee,
            revert_error,
            messages_sent,
            execution_resources: actual_resources,
        }),

        TxRef::Declare(_) => Receipt::Declare(DeclareTxReceipt {
            events,
            fee,
            revert_error,
            messages_sent,
            execution_resources: actual_resources,
        }),

        TxRef::L1Handler(tx) => Receipt::L1Handler(L1HandlerTxReceipt {
            events,
            fee,
            revert_error,
            messages_sent,
            message_hash: tx.message_hash,
            execution_resources: actual_resources,
        }),

        TxRef::DeployAccount(tx) => Receipt::DeployAccount(DeployAccountTxReceipt {
            events,
            fee,
            revert_error,
            messages_sent,
            execution_resources: actual_resources,
            contract_address: tx.contract_address(),
        }),
    }
}

pub fn events_from_exec_info(info: &TxExecInfo) -> Vec<Event> {
    let mut events: Vec<Event> = vec![];

    if let Some(ref call) = info.validate_call_info {
        events.extend(get_events_recur(call));
    }

    if let Some(ref call) = info.execute_call_info {
        events.extend(get_events_recur(call));
    }

    if let Some(ref call) = info.fee_transfer_call_info {
        events.extend(get_events_recur(call));
    }

    events
}

pub fn l2_to_l1_messages_from_exec_info(info: &TxExecInfo) -> Vec<MessageToL1> {
    let mut messages = vec![];

    if let Some(ref info) = info.validate_call_info {
        messages.extend(get_l2_to_l1_messages_recur(info));
    }

    if let Some(ref info) = info.execute_call_info {
        messages.extend(get_l2_to_l1_messages_recur(info));
    }

    if let Some(ref info) = info.fee_transfer_call_info {
        messages.extend(get_l2_to_l1_messages_recur(info));
    }

    messages
}

fn get_events_recur(info: &CallInfo) -> Vec<Event> {
    let mut events: Vec<Event> = vec![];

    events.extend(info.events.iter().map(|e| Event {
        from_address: info.contract_address,
        data: e.data.clone(),
        keys: e.keys.clone(),
    }));

    info.inner_calls.iter().for_each(|call| {
        events.extend(get_events_recur(call));
    });

    events
}

fn get_l2_to_l1_messages_recur(info: &CallInfo) -> Vec<MessageToL1> {
    let mut messages = vec![];

    messages.extend(info.l2_to_l1_messages.iter().map(|m| MessageToL1 {
        from_address: m.from_address,
        to_address: m.to_address,
        payload: m.payload.clone(),
    }));

    info.inner_calls.iter().for_each(|call| {
        messages.extend(get_l2_to_l1_messages_recur(call));
    });

    messages
}

#[cfg(test)]
mod tests {
    use katana_primitives::event::OrderedEvent;
    use katana_primitives::message::OrderedL2ToL1Message;
    use katana_primitives::receipt::{Event, MessageToL1};
    use katana_primitives::trace::CallInfo;
    use starknet::macros::felt;

    fn call_info() -> CallInfo {
        let inner_calls = vec![CallInfo {
            contract_address: felt!("0x111").into(),
            events: vec![
                OrderedEvent { order: 1, data: vec![1u8.into()], keys: vec![10u8.into()] },
                OrderedEvent { order: 4, data: vec![2u8.into()], keys: vec![20u8.into()] },
            ],
            l2_to_l1_messages: vec![OrderedL2ToL1Message {
                order: 0,
                from_address: felt!("0x111").into(),
                to_address: felt!("0x200"),
                payload: vec![1u8.into()],
            }],
            ..Default::default()
        }];

        CallInfo {
            contract_address: felt!("0x100").into(),
            events: vec![OrderedEvent { order: 0, data: vec![1u8.into()], keys: vec![2u8.into()] }],
            l2_to_l1_messages: vec![
                OrderedL2ToL1Message {
                    order: 0,
                    from_address: felt!("0x100").into(),
                    to_address: felt!("0x200"),
                    payload: vec![1u8.into()],
                },
                OrderedL2ToL1Message {
                    order: 1,
                    from_address: felt!("0x100").into(),
                    to_address: felt!("0x201"),
                    payload: vec![2u8.into()],
                },
            ],
            inner_calls,
            ..Default::default()
        }
    }

    #[test]
    fn get_events_from_exec_info() {
        let info = call_info();
        let events = super::get_events_recur(&info);

        let expected_events = vec![
            Event {
                from_address: info.contract_address,
                data: vec![1u8.into()],
                keys: vec![2u8.into()],
            },
            Event {
                from_address: info.inner_calls[0].contract_address,
                data: vec![1u8.into()],
                keys: vec![10u8.into()],
            },
            Event {
                from_address: info.inner_calls[0].contract_address,
                data: vec![2u8.into()],
                keys: vec![20u8.into()],
            },
        ];

        similar_asserts::assert_eq!(events, expected_events)
    }

    #[test]
    fn get_l2_to_l1_messages_from_exec_info() {
        let info = call_info();
        let events = super::get_l2_to_l1_messages_recur(&info);

        // TODO: Maybe remove `from_address` from `MessageToL1`?
        //
        // The from address is not constrained to be the same as the contract address
        // of the call info beca use we already set it when converting TxExecInfo from its executor
        // specific counterparts. Which is different compare to the events where it doesn't have
        // from address field in `OrderedEvent`.
        let expected_messages = vec![
            MessageToL1 {
                from_address: info.contract_address,
                to_address: info.l2_to_l1_messages[0].to_address,
                payload: info.l2_to_l1_messages[0].payload.clone(),
            },
            MessageToL1 {
                from_address: info.contract_address,
                to_address: info.l2_to_l1_messages[1].to_address,
                payload: info.l2_to_l1_messages[1].payload.clone(),
            },
            MessageToL1 {
                from_address: info.inner_calls[0].contract_address,
                to_address: info.inner_calls[0].l2_to_l1_messages[0].to_address,
                payload: info.inner_calls[0].l2_to_l1_messages[0].payload.clone(),
            },
        ];

        similar_asserts::assert_eq!(events, expected_messages)
    }
}
