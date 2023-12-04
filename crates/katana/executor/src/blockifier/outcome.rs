use blockifier::transaction::objects::TransactionExecutionInfo;
use katana_primitives::receipt::{
    DeclareTxReceipt, DeployAccountTxReceipt, InvokeTxReceipt, L1HandlerTxReceipt, Receipt,
};
use katana_primitives::transaction::Tx;

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
        let actual_resources = execution_info.actual_resources.0.clone();

        let receipt = match tx.as_ref() {
            Tx::Invoke(_) => Receipt::Invoke(InvokeTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                actual_resources,
            }),

            Tx::Declare(_) => Receipt::Declare(DeclareTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                actual_resources,
            }),

            Tx::L1Handler(_) => Receipt::L1Handler(L1HandlerTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                actual_resources,
            }),

            Tx::DeployAccount(tx) => Receipt::DeployAccount(DeployAccountTxReceipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                actual_resources,
                contract_address: tx.contract_address,
            }),
        };

        Self { receipt, execution_info }
    }
}
