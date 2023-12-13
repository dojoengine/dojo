use katana_primitives::block::{BlockHash, BlockNumber, FinalityStatus};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::TxHash;
use serde::Serialize;
use starknet::core::types::{
    DeclareTransactionReceipt, DeployAccountTransactionReceipt, ExecutionResult,
    InvokeTransactionReceipt, L1HandlerTransactionReceipt, PendingDeclareTransactionReceipt,
    PendingDeployAccountTransactionReceipt, PendingInvokeTransactionReceipt,
    PendingL1HandlerTransactionReceipt, PendingTransactionReceipt, TransactionFinalityStatus,
    TransactionReceipt,
};

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct TxReceipt(starknet::core::types::TransactionReceipt);

impl TxReceipt {
    pub fn new(
        transaction_hash: TxHash,
        block_number: BlockNumber,
        block_hash: BlockHash,
        finality_status: FinalityStatus,
        receipt: Receipt,
    ) -> Self {
        let finality_status = match finality_status {
            FinalityStatus::AcceptedOnL1 => TransactionFinalityStatus::AcceptedOnL1,
            FinalityStatus::AcceptedOnL2 => TransactionFinalityStatus::AcceptedOnL2,
        };

        let receipt = match receipt {
            Receipt::Invoke(rct) => TransactionReceipt::Invoke(InvokeTransactionReceipt {
                block_hash,
                block_number,
                finality_status,
                transaction_hash,
                events: rct.events,
                messages_sent: rct.messages_sent,
                actual_fee: rct.actual_fee.into(),
                execution_resources: rct.execution_resources,
                execution_result: if let Some(reason) = rct.revert_error {
                    ExecutionResult::Reverted { reason }
                } else {
                    ExecutionResult::Succeeded
                },
            }),

            Receipt::Declare(rct) => TransactionReceipt::Declare(DeclareTransactionReceipt {
                block_hash,
                block_number,
                finality_status,
                transaction_hash,
                events: rct.events,
                messages_sent: rct.messages_sent,
                actual_fee: rct.actual_fee.into(),
                execution_resources: rct.execution_resources,
                execution_result: if let Some(reason) = rct.revert_error {
                    ExecutionResult::Reverted { reason }
                } else {
                    ExecutionResult::Succeeded
                },
            }),

            Receipt::L1Handler(rct) => TransactionReceipt::L1Handler(L1HandlerTransactionReceipt {
                block_hash,
                block_number,
                finality_status,
                transaction_hash,
                events: rct.events,
                message_hash: rct.message_hash,
                messages_sent: rct.messages_sent,
                actual_fee: rct.actual_fee.into(),
                execution_resources: rct.execution_resources,
                execution_result: if let Some(reason) = rct.revert_error {
                    ExecutionResult::Reverted { reason }
                } else {
                    ExecutionResult::Succeeded
                },
            }),

            Receipt::DeployAccount(rct) => {
                TransactionReceipt::DeployAccount(DeployAccountTransactionReceipt {
                    block_hash,
                    block_number,
                    finality_status,
                    transaction_hash,
                    events: rct.events,
                    messages_sent: rct.messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: rct.execution_resources,
                    contract_address: rct.contract_address.into(),
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }
        };

        Self(receipt)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct PendingTxReceipt(starknet::core::types::PendingTransactionReceipt);

impl PendingTxReceipt {
    pub fn new(transaction_hash: TxHash, receipt: Receipt) -> Self {
        let receipt = match receipt {
            Receipt::Invoke(rct) => {
                PendingTransactionReceipt::Invoke(PendingInvokeTransactionReceipt {
                    transaction_hash,
                    events: rct.events,
                    messages_sent: rct.messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: rct.execution_resources,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::Declare(rct) => {
                PendingTransactionReceipt::Declare(PendingDeclareTransactionReceipt {
                    transaction_hash,
                    events: rct.events,
                    messages_sent: rct.messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: rct.execution_resources,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::L1Handler(rct) => {
                PendingTransactionReceipt::L1Handler(PendingL1HandlerTransactionReceipt {
                    transaction_hash,
                    events: rct.events,
                    message_hash: rct.message_hash,
                    messages_sent: rct.messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: rct.execution_resources,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::DeployAccount(rct) => {
                PendingTransactionReceipt::DeployAccount(PendingDeployAccountTransactionReceipt {
                    transaction_hash,
                    events: rct.events,
                    messages_sent: rct.messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    contract_address: rct.contract_address.into(),
                    execution_resources: rct.execution_resources,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }
        };

        Self(receipt)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MaybePendingTxReceipt {
    Receipt(TxReceipt),
    Pending(PendingTxReceipt),
}

impl From<starknet::core::types::TransactionReceipt> for TxReceipt {
    fn from(receipt: starknet::core::types::TransactionReceipt) -> Self {
        Self(receipt)
    }
}
