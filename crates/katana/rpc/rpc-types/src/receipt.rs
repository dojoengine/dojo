use katana_primitives::block::FinalityStatus;
use katana_primitives::fee::TxFeeInfo;
use katana_primitives::receipt::{MessageToL1, Receipt, TxExecutionResources};
use katana_primitives::transaction::TxHash;
use serde::{Deserialize, Serialize};
pub use starknet::core::types::ReceiptBlock;
use starknet::core::types::{
    ComputationResources, DataAvailabilityResources, DataResources, DeclareTransactionReceipt,
    DeployAccountTransactionReceipt, ExecutionResult, FeePayment, Hash256,
    InvokeTransactionReceipt, L1HandlerTransactionReceipt, TransactionFinalityStatus,
    TransactionReceipt, TransactionReceiptWithBlockInfo,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TxReceipt(pub(crate) starknet::core::types::TransactionReceipt);

impl TxReceipt {
    pub fn new(
        transaction_hash: TxHash,
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
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: to_rpc_fee(rct.fee),
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
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: to_rpc_fee(rct.fee),
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
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: to_rpc_fee(rct.fee),
                    execution_resources: ExecutionResources::from(rct.execution_resources).0,
                    message_hash: Hash256::from_bytes(*rct.message_hash),
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
                    messages_sent,
                    finality_status,
                    transaction_hash,
                    actual_fee: to_rpc_fee(rct.fee),
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
pub struct TxReceiptWithBlockInfo(starknet::core::types::TransactionReceiptWithBlockInfo);

impl TxReceiptWithBlockInfo {
    pub fn new(
        block: ReceiptBlock,
        transaction_hash: TxHash,
        finality_status: FinalityStatus,
        receipt: Receipt,
    ) -> Self {
        let receipt = TxReceipt::new(transaction_hash, finality_status, receipt).0;
        Self(TransactionReceiptWithBlockInfo { receipt, block })
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
            computation_resources: ComputationResources {
                steps: value.steps,
                memory_holes: value.memory_holes,
                ec_op_builtin_applications: value.ec_op_builtin,
                ecdsa_builtin_applications: value.ecdsa_builtin,
                keccak_builtin_applications: value.keccak_builtin,
                bitwise_builtin_applications: value.bitwise_builtin,
                pedersen_builtin_applications: value.pedersen_builtin,
                poseidon_builtin_applications: value.poseidon_builtin,
                range_check_builtin_applications: value.range_check_builtin,
                segment_arena_builtin: value.segment_arena_builtin,
            },
            data_resources: DataResources {
                data_availability: DataAvailabilityResources { l1_data_gas: 0, l1_gas: 0 },
            },
        })
    }
}

fn to_rpc_fee(fee: TxFeeInfo) -> FeePayment {
    FeePayment { amount: fee.overall_fee.into(), unit: fee.unit }
}
