use std::sync::Arc;

use starknet::core::types::{
    DeclareTransactionReceipt, DeployAccountTransactionReceipt, DeployTransactionReceipt, Event,
    FieldElement, InvokeTransactionReceipt, L1HandlerTransactionReceipt, MsgToL1,
    PendingDeclareTransactionReceipt, PendingDeployAccountTransactionReceipt,
    PendingDeployTransactionReceipt, PendingInvokeTransactionReceipt,
    PendingL1HandlerTransactionReceipt, PendingTransactionReceipt as RpcPendingTransactionReceipt,
    Transaction as RpcTransaction, TransactionReceipt as RpcTransactionReceipt,
    TransactionStatus as RpcTransactionStatus,
};
use starknet::core::utils::get_contract_address;
use starknet_api::transaction::Transaction as ApiTransaction;

use crate::backend::executor::ExecutedTransaction;
use crate::utils::transaction::convert_api_to_rpc_tx;

/// The status of the transactions known to the sequencer.
#[derive(Debug, Clone, Copy)]
pub enum TransactionStatus {
    /// Transaction executed unsuccessfully and thus was skipped.
    Rejected,
    /// When the transaction pass validation but encountered error during execution.
    Reverted,
    /// Transactions that have been included in the L2 block which have
    /// passed both validation and execution.
    AcceptedOnL2,
    /// When the block of which the transaction is included in have been committed to the
    /// L1 settlement layer.
    AcceptedOnL1,
}

/// Represents all transactions that are known to the sequencer.
#[derive(Debug, Clone)]
pub enum KnownTransaction {
    Pending(PendingTransaction),
    Included(IncludedTransaction),
    Rejected(Box<RejectedTransaction>),
}

impl KnownTransaction {
    pub fn is_rejected(&self) -> bool {
        matches!(self, KnownTransaction::Rejected(_))
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, KnownTransaction::Pending(_))
    }

    pub fn is_included(&self) -> bool {
        matches!(self, KnownTransaction::Included(_))
    }
}

#[derive(Debug, Clone)]
pub struct PendingTransaction(pub Arc<ExecutedTransaction>);

/// A transaction that is known to be included in a block. Which also includes
/// reverted transactions and transactions that are currently in the `pending` block.
#[derive(Debug, Clone)]
pub struct IncludedTransaction {
    pub block_number: u64,
    pub block_hash: FieldElement,
    pub transaction: Arc<ExecutedTransaction>,
    pub status: TransactionStatus,
}

/// A transaction that is known to be rejected by the sequencer i.e.,
/// transaction that didn't pass the validation logic.
#[derive(Debug, Clone)]
pub struct RejectedTransaction {
    pub transaction: ApiTransaction,
    pub execution_error: String,
}

#[derive(Debug, Clone)]
pub struct TransactionOutput {
    pub actual_fee: u128,
    pub events: Vec<Event>,
    pub messages_sent: Vec<MsgToL1>,
}

impl IncludedTransaction {
    pub fn receipt(&self) -> RpcTransactionReceipt {
        match &self.transaction.transaction {
            ApiTransaction::Invoke(tx) => RpcTransactionReceipt::Invoke(InvokeTransactionReceipt {
                status: self.status.into(),
                block_hash: self.block_hash,
                block_number: self.block_number,
                events: self.transaction.output.events.clone(),
                messages_sent: self.transaction.output.messages_sent.clone(),
                transaction_hash: tx.transaction_hash().0.into(),
                actual_fee: self.transaction.execution_info.actual_fee.0.into(),
            }),

            ApiTransaction::Declare(tx) => {
                RpcTransactionReceipt::Declare(DeclareTransactionReceipt {
                    status: self.status.into(),
                    block_hash: self.block_hash,
                    block_number: self.block_number,
                    events: self.transaction.output.events.clone(),
                    transaction_hash: tx.transaction_hash().0.into(),
                    messages_sent: self.transaction.output.messages_sent.clone(),
                    actual_fee: self.transaction.execution_info.actual_fee.0.into(),
                })
            }

            ApiTransaction::DeployAccount(tx) => {
                RpcTransactionReceipt::DeployAccount(DeployAccountTransactionReceipt {
                    status: self.status.into(),
                    block_hash: self.block_hash,
                    block_number: self.block_number,
                    events: self.transaction.output.events.clone(),
                    transaction_hash: tx.transaction_hash.0.into(),
                    // TODO: store the contract address instead of computing everytime
                    contract_address: get_contract_address(
                        tx.contract_address_salt.0.into(),
                        tx.class_hash.0.into(),
                        &tx.constructor_calldata.0.iter().map(|f| (*f).into()).collect::<Vec<_>>(),
                        FieldElement::ZERO,
                    ),
                    messages_sent: self.transaction.output.messages_sent.clone(),
                    actual_fee: self.transaction.execution_info.actual_fee.0.into(),
                })
            }

            ApiTransaction::L1Handler(tx) => {
                RpcTransactionReceipt::L1Handler(L1HandlerTransactionReceipt {
                    status: self.status.into(),
                    block_hash: self.block_hash,
                    block_number: self.block_number,
                    events: self.transaction.output.events.clone(),
                    transaction_hash: tx.transaction_hash.0.into(),
                    messages_sent: self.transaction.output.messages_sent.clone(),
                    actual_fee: self.transaction.execution_info.actual_fee.0.into(),
                })
            }

            ApiTransaction::Deploy(tx) => RpcTransactionReceipt::Deploy(DeployTransactionReceipt {
                status: self.status.into(),
                block_hash: self.block_hash,
                block_number: self.block_number,
                events: self.transaction.output.events.clone(),
                transaction_hash: tx.transaction_hash.0.into(),
                // TODO: store the contract address instead of computing everytime
                contract_address: get_contract_address(
                    tx.contract_address_salt.0.into(),
                    tx.class_hash.0.into(),
                    &tx.constructor_calldata.0.iter().map(|f| (*f).into()).collect::<Vec<_>>(),
                    FieldElement::ZERO,
                ),
                messages_sent: self.transaction.output.messages_sent.clone(),
                actual_fee: self.transaction.execution_info.actual_fee.0.into(),
            }),
        }
    }
}

impl PendingTransaction {
    pub fn receipt(&self) -> RpcPendingTransactionReceipt {
        match &self.0.transaction {
            ApiTransaction::Invoke(tx) => {
                RpcPendingTransactionReceipt::Invoke(PendingInvokeTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: tx.transaction_hash().0.into(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }

            ApiTransaction::Declare(tx) => {
                RpcPendingTransactionReceipt::Declare(PendingDeclareTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: tx.transaction_hash().0.into(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }

            ApiTransaction::DeployAccount(tx) => RpcPendingTransactionReceipt::DeployAccount(
                PendingDeployAccountTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: tx.transaction_hash.0.into(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                },
            ),

            ApiTransaction::L1Handler(tx) => {
                RpcPendingTransactionReceipt::L1Handler(PendingL1HandlerTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: tx.transaction_hash.0.into(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }

            ApiTransaction::Deploy(tx) => {
                RpcPendingTransactionReceipt::Deploy(PendingDeployTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: tx.transaction_hash.0.into(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                    // TODO: store the contract address instead of computing everytime
                    contract_address: get_contract_address(
                        tx.contract_address_salt.0.into(),
                        tx.class_hash.0.into(),
                        &tx.constructor_calldata.0.iter().map(|f| (*f).into()).collect::<Vec<_>>(),
                        FieldElement::ZERO,
                    ),
                })
            }
        }
    }
}

impl KnownTransaction {
    pub fn status(&self) -> TransactionStatus {
        match self {
            KnownTransaction::Pending(_) => TransactionStatus::AcceptedOnL2,
            KnownTransaction::Rejected(_) => TransactionStatus::Rejected,
            KnownTransaction::Included(tx) => tx.status,
        }
    }
}

impl From<TransactionStatus> for RpcTransactionStatus {
    fn from(status: TransactionStatus) -> Self {
        match status {
            TransactionStatus::AcceptedOnL2 => RpcTransactionStatus::AcceptedOnL2,
            TransactionStatus::AcceptedOnL1 => RpcTransactionStatus::AcceptedOnL1,
            TransactionStatus::Rejected => RpcTransactionStatus::Rejected,
            // TODO: change this to `REVERTED` once the status is implemented in `starknet-rs`
            TransactionStatus::Reverted => RpcTransactionStatus::AcceptedOnL2,
        }
    }
}

impl From<PendingTransaction> for KnownTransaction {
    fn from(transaction: PendingTransaction) -> Self {
        KnownTransaction::Pending(transaction)
    }
}

impl From<IncludedTransaction> for KnownTransaction {
    fn from(transaction: IncludedTransaction) -> Self {
        KnownTransaction::Included(transaction)
    }
}

impl From<RejectedTransaction> for KnownTransaction {
    fn from(transaction: RejectedTransaction) -> Self {
        KnownTransaction::Rejected(Box::new(transaction))
    }
}

impl From<KnownTransaction> for RpcTransaction {
    fn from(transaction: KnownTransaction) -> Self {
        match transaction {
            KnownTransaction::Pending(tx) => convert_api_to_rpc_tx(tx.0.transaction.clone()),
            KnownTransaction::Rejected(tx) => convert_api_to_rpc_tx(tx.transaction),
            KnownTransaction::Included(tx) => {
                convert_api_to_rpc_tx(tx.transaction.transaction.clone())
            }
        }
    }
}
