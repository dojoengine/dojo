use std::collections::HashMap;

use blockifier::transaction::objects::TransactionExecutionInfo;
use katana_primitives::receipt::{
    DeclareTxReceipt, DeployAccountTxReceipt, InvokeTxReceipt, L1HandlerTxReceipt, Receipt,
};
use katana_primitives::transaction::Tx;
use starknet::core::types::ExecutionResources;

use super::utils::{events_from_exec_info, l2_to_l1_messages_from_exec_info};

pub struct TxReceiptWithExecInfo {
    pub receipt: Receipt,
    pub execution_info: TransactionExecutionInfo,
}

impl TxReceiptWithExecInfo {
    pub fn from_tx_exec_result(
        tx: impl AsRef<Tx>,
        execution_info: TransactionExecutionInfo,
    ) -> Self {
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

            Tx::L1Handler(_) => Receipt::L1Handler(L1HandlerTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
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

fn parse_actual_resources(resources: &HashMap<String, usize>) -> ExecutionResources {
    ExecutionResources {
        steps: resources.get("n_steps").copied().unwrap_or_default() as u64,
        memory_holes: resources.get("memory_holes").map(|x| *x as u64),
        ec_op_builtin_applications: resources.get("ec_op_builtin").copied().unwrap_or_default()
            as u64,
        ecdsa_builtin_applications: resources.get("ecdsa_builtin").copied().unwrap_or_default()
            as u64,
        keccak_builtin_applications: resources.get("keccak_builtin").copied().unwrap_or_default()
            as u64,
        bitwise_builtin_applications: resources.get("bitwise_builtin").copied().unwrap_or_default()
            as u64,
        pedersen_builtin_applications: resources
            .get("pedersen_builtin")
            .copied()
            .unwrap_or_default() as u64,
        poseidon_builtin_applications: resources
            .get("poseidon_builtin")
            .copied()
            .unwrap_or_default() as u64,
        range_check_builtin_applications: resources
            .get("range_check_builtin")
            .copied()
            .unwrap_or_default() as u64,
    }
}
