use std::collections::HashMap;

use blockifier::transaction::objects::TransactionExecutionInfo;
use katana_primitives::receipt::{
    DeclareTxReceipt, DeployAccountTxReceipt, InvokeTxReceipt, L1HandlerTxReceipt, Receipt,
    TxExecutionResources,
};
use katana_primitives::transaction::Tx;

use super::utils::{events_from_exec_info, l2_to_l1_messages_from_exec_info};

pub struct TxReceiptWithExecInfo {
    pub receipt: Receipt,
    pub execution_info: TransactionExecutionInfo,
}

impl TxReceiptWithExecInfo {
    pub fn new(tx: impl AsRef<Tx>, execution_info: TransactionExecutionInfo) -> Self {
        let actual_fee = execution_info.actual_fee.0;
        let events = events_from_exec_info(&execution_info);
        let revert_error = execution_info.revert_error.clone();
        let messages_sent = l2_to_l1_messages_from_exec_info(&execution_info);
        let actual_resources = parse_actual_resources(&execution_info.actual_resources.0);

        let receipt = match tx.as_ref() {
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
                contract_address: tx.contract_address,
            }),
        };

        Self { receipt, execution_info }
    }
}

/// Parse the `actual resources` field from the execution info into a more structured type,
/// [`TxExecutionResources`].
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
