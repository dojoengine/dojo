use std::collections::HashMap;

use katana_primitives::receipt::{
    DeclareTxReceipt, DeployAccountTxReceipt, Event, InvokeTxReceipt, L1HandlerTxReceipt,
    MessageToL1, Receipt, TxExecutionResources,
};
use katana_primitives::transaction::Tx;
use sir::execution::CallInfo;

use super::utils;
use crate::abstraction::TransactionExecutionOutput;

#[derive(Debug, Default)]
pub struct TransactionExecutionInfo {
    pub gas_used: u128,
    pub(super) inner: sir::execution::TransactionExecutionInfo,
}

impl TransactionExecutionOutput for TransactionExecutionInfo {
    fn receipt(&self, tx: &Tx) -> Receipt {
        let actual_fee = self.inner.actual_fee;
        let revert_error = self.inner.revert_error.clone();

        let events = events_from_exec_info(self);
        let messages_sent = l2_to_l1_messages_from_exec_info(self);
        let execution_resources = parse_actual_resources(&self.inner.actual_resources);

        match tx {
            Tx::Invoke(_) => Receipt::Invoke(InvokeTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                execution_resources,
            }),

            Tx::Declare(_) => Receipt::Declare(DeclareTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                execution_resources,
            }),

            Tx::L1Handler(tx) => Receipt::L1Handler(L1HandlerTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                execution_resources,
                message_hash: tx.message_hash,
            }),

            Tx::DeployAccount(tx) => Receipt::DeployAccount(DeployAccountTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                execution_resources,
                contract_address: tx.contract_address,
            }),
        }
    }

    fn actual_fee(&self) -> u128 {
        self.inner.actual_fee
    }

    fn gas_used(&self) -> u128 {
        self.gas_used
    }

    fn revert_error(&self) -> Option<&str> {
        self.inner.revert_error.as_deref()
    }
}

fn events_from_exec_info(info: &TransactionExecutionInfo) -> Vec<Event> {
    let mut events: Vec<Event> = vec![];

    fn get_events_recursively(call_info: &CallInfo) -> Vec<Event> {
        let mut events: Vec<Event> = vec![];

        events.extend(call_info.events.iter().map(|e| Event {
            from_address: utils::to_address(&call_info.contract_address),
            data: e.data.iter().map(utils::to_felt).collect(),
            keys: e.keys.iter().map(utils::to_felt).collect(),
        }));

        call_info.internal_calls.iter().for_each(|call| {
            events.extend(get_events_recursively(call));
        });

        events
    }

    if let Some(ref call) = info.inner.validate_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = info.inner.call_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = info.inner.fee_transfer_info {
        events.extend(get_events_recursively(call));
    }

    events
}

fn l2_to_l1_messages_from_exec_info(info: &TransactionExecutionInfo) -> Vec<MessageToL1> {
    let mut messages = vec![];

    fn get_messages_recursively(info: &CallInfo) -> Vec<MessageToL1> {
        let mut messages = vec![];

        // By default, `from_address` must correspond to the contract address that
        // is sending the message. In the case of library calls, `code_address` is `None`,
        // we then use the `caller_address` instead (which can also be an account).
        let from_address = if let Some(ref code_address) = info.code_address {
            utils::to_address(code_address)
        } else {
            utils::to_address(&info.caller_address)
        };

        messages.extend(info.l2_to_l1_messages.iter().map(|m| MessageToL1 {
            from_address,
            to_address: utils::to_felt(&m.to_address.0),
            payload: m.payload.iter().map(utils::to_felt).collect(),
        }));

        info.internal_calls.iter().for_each(|call| {
            messages.extend(get_messages_recursively(call));
        });

        messages
    }

    if let Some(ref info) = info.inner.validate_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = info.inner.call_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = info.inner.fee_transfer_info {
        messages.extend(get_messages_recursively(info));
    }

    messages
}

fn parse_actual_resources(resources: &HashMap<String, usize>) -> TxExecutionResources {
    TxExecutionResources {
        steps: resources.get("n_steps").copied().unwrap_or_default() as u64,
        memory_holes: resources.get("memory_holes").map(|x| *x as u64),
        ec_op_builtin: resources.get("ec_op_builtin").map(|x| *x as u64),
        ecdsa_builtin: resources.get("ecdsa_builtin").map(|x| *x as u64),
        keccak_builtin: resources.get("keccak_builtin").map(|x| *x as u64),
        bitwise_builtin: resources.get("bitwise_builtin").map(|x| *x as u64),
        pedersen_builtin: resources.get("pedersen_builtin").map(|x| *x as u64),
        poseidon_builtin: resources.get("poseidon_builtin").map(|x| *x as u64),
        range_check_builtin: resources.get("range_check_builtin").map(|x| *x as u64),
    }
}
