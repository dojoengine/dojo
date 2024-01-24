use katana_primitives::block::{BlockHash, BlockNumber, FinalityStatus};
use katana_primitives::receipt::{MessageToL1, Receipt, TxExecutionResources};
use katana_primitives::transaction::TxHash;
use serde::{Deserialize, Serialize};
use starknet::core::types::{
    DeclareTransactionReceipt, DeployAccountTransactionReceipt, ExecutionResult, Hash256,
    InvokeTransactionReceipt, L1HandlerTransactionReceipt, PendingDeclareTransactionReceipt,
    PendingDeployAccountTransactionReceipt, PendingInvokeTransactionReceipt,
    PendingL1HandlerTransactionReceipt, PendingTransactionReceipt, TransactionFinalityStatus,
    TransactionReceipt,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            Receipt::Invoke(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                TransactionReceipt::Invoke(InvokeTransactionReceipt {
                    events,
                    block_hash,
                    block_number,
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::Declare(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                TransactionReceipt::Declare(DeclareTransactionReceipt {
                    events,
                    block_hash,
                    block_number,
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::L1Handler(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                TransactionReceipt::L1Handler(L1HandlerTransactionReceipt {
                    events,
                    block_hash,
                    block_number,
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
                    message_hash: Hash256::from_bytes(rct.message_hash.to_fixed_bytes()),
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::DeployAccount(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                TransactionReceipt::DeployAccount(DeployAccountTransactionReceipt {
                    events,
                    block_hash,
                    block_number,
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: rct.actual_fee.into(),
                    contract_address: rct.contract_address.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PendingTxReceipt(starknet::core::types::PendingTransactionReceipt);

impl PendingTxReceipt {
    pub fn new(transaction_hash: TxHash, receipt: Receipt) -> Self {
        let receipt = match receipt {
            Receipt::Invoke(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                PendingTransactionReceipt::Invoke(PendingInvokeTransactionReceipt {
                    transaction_hash,
                    events,
                    messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::Declare(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                PendingTransactionReceipt::Declare(PendingDeclareTransactionReceipt {
                    events,
                    transaction_hash,
                    messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::L1Handler(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                PendingTransactionReceipt::L1Handler(PendingL1HandlerTransactionReceipt {
                    transaction_hash,
                    events,
                    messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
                    message_hash: Hash256::from_bytes(rct.message_hash.0),
                    execution_result: if let Some(reason) = rct.revert_error {
                        ExecutionResult::Reverted { reason }
                    } else {
                        ExecutionResult::Succeeded
                    },
                })
            }

            Receipt::DeployAccount(rct) => {
                let messages_sent =
                    rct.messages_sent.into_iter().map(|e| MsgToL1::from(e).0).collect();
                let events = rct.events.into_iter().map(|e| Event::from(e).0).collect();

                PendingTransactionReceipt::DeployAccount(PendingDeployAccountTransactionReceipt {
                    transaction_hash,
                    events,
                    messages_sent,
                    actual_fee: rct.actual_fee.into(),
                    contract_address: rct.contract_address.into(),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

struct MsgToL1(starknet::core::types::MsgToL1);

impl From<MessageToL1> for MsgToL1 {
    fn from(value: MessageToL1) -> Self {
        MsgToL1(starknet::core::types::MsgToL1 {
            from_address: value.from_address.into(),
            to_address: value.to_address,
            payload: value.payload,
        })
    }
}

struct Event(starknet::core::types::Event);

impl From<katana_primitives::receipt::Event> for Event {
    fn from(value: katana_primitives::receipt::Event) -> Self {
        Event(starknet::core::types::Event {
            from_address: value.from_address.into(),
            keys: value.keys,
            data: value.data,
        })
    }
}

struct ExecutionResources(starknet::core::types::ExecutionResources);

impl From<TxExecutionResources> for ExecutionResources {
    fn from(value: TxExecutionResources) -> Self {
        ExecutionResources(starknet::core::types::ExecutionResources {
            steps: value.steps,
            memory_holes: value.memory_holes,
            ec_op_builtin_applications: value.ec_op_builtin.unwrap_or_default(),
            ecdsa_builtin_applications: value.ecdsa_builtin.unwrap_or_default(),
            keccak_builtin_applications: value.keccak_builtin.unwrap_or_default(),
            bitwise_builtin_applications: value.bitwise_builtin.unwrap_or_default(),
            pedersen_builtin_applications: value.pedersen_builtin.unwrap_or_default(),
            poseidon_builtin_applications: value.poseidon_builtin.unwrap_or_default(),
            range_check_builtin_applications: value.range_check_builtin.unwrap_or_default(),
        })
    }
}
