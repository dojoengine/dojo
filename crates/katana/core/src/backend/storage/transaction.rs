use std::sync::Arc;

use blockifier::execution::contract_class::ContractClass;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transactions::{
    DeclareTransaction as ExecutionDeclareTransaction,
    DeployAccountTransaction as ExecutionDeployAccountTransaction,
};
use starknet::core::types::{
    DeclareTransactionReceipt, DeployAccountTransactionReceipt, Event, FieldElement,
    FlattenedSierraClass, InvokeTransactionReceipt, MsgToL1, PendingDeclareTransactionReceipt,
    PendingDeployAccountTransactionReceipt, PendingInvokeTransactionReceipt, L1HandlerTransactionReceipt, PendingL1HandlerTransactionReceipt,
    PendingTransactionReceipt as RpcPendingTransactionReceipt, Transaction as RpcTransaction,
    TransactionReceipt as RpcTransactionReceipt, TransactionStatus as RpcTransactionStatus,
};
use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::transaction::{
    DeclareTransaction as ApiDeclareTransaction,
    DeployAccountTransaction as ApiDeployAccountTransaction,
    InvokeTransaction as ApiInvokeTransaction, Transaction as ApiTransaction,
    L1HandlerTransaction as ApiL1HandlerTransaction,
};

use crate::backend::executor::ExecutedTransaction;
use crate::utils::transaction::api_to_rpc_transaction;

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

#[derive(Debug, Clone)]
pub enum Transaction {
    Invoke(InvokeTransaction),
    Declare(DeclareTransaction),
    DeployAccount(DeployAccountTransaction),
    L1Handler(L1HandlerTransaction),
}

impl Transaction {
    pub fn hash(&self) -> FieldElement {
        match self {
            Transaction::Invoke(tx) => tx.0.transaction_hash().0.into(),
            Transaction::Declare(tx) => tx.inner.transaction_hash().0.into(),
            Transaction::DeployAccount(tx) => tx.inner.transaction_hash.0.into(),
            Transaction::L1Handler(tx) => tx.0.transaction_hash.0.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct L1HandlerTransaction(pub ApiL1HandlerTransaction);

#[derive(Debug, Clone)]
pub struct InvokeTransaction(pub ApiInvokeTransaction);

#[derive(Debug, Clone)]
pub struct DeclareTransaction {
    pub inner: ApiDeclareTransaction,
    pub compiled_class: ContractClass,
    pub sierra_class: Option<FlattenedSierraClass>,
}

#[derive(Debug, Clone)]
pub struct DeployAccountTransaction {
    pub inner: ApiDeployAccountTransaction,
    pub contract_address: FieldElement,
}

impl IncludedTransaction {
    pub fn receipt(&self) -> RpcTransactionReceipt {
        match &self.transaction.inner {
            Transaction::Invoke(_) => RpcTransactionReceipt::Invoke(InvokeTransactionReceipt {
                status: self.status.into(),
                block_hash: self.block_hash,
                block_number: self.block_number,
                events: self.transaction.output.events.clone(),
                messages_sent: self.transaction.output.messages_sent.clone(),
                transaction_hash: self.transaction.inner.hash(),
                actual_fee: self.transaction.execution_info.actual_fee.0.into(),
            }),

            Transaction::Declare(_) => RpcTransactionReceipt::Declare(DeclareTransactionReceipt {
                status: self.status.into(),
                block_hash: self.block_hash,
                block_number: self.block_number,
                events: self.transaction.output.events.clone(),
                transaction_hash: self.transaction.inner.hash(),
                messages_sent: self.transaction.output.messages_sent.clone(),
                actual_fee: self.transaction.execution_info.actual_fee.0.into(),
            }),

            Transaction::DeployAccount(tx) => {
                RpcTransactionReceipt::DeployAccount(DeployAccountTransactionReceipt {
                    status: self.status.into(),
                    block_hash: self.block_hash,
                    block_number: self.block_number,
                    contract_address: tx.contract_address,
                    events: self.transaction.output.events.clone(),
                    transaction_hash: self.transaction.inner.hash(),
                    messages_sent: self.transaction.output.messages_sent.clone(),
                    actual_fee: self.transaction.execution_info.actual_fee.0.into(),
                })
            },

            Transaction::L1Handler(_) => RpcTransactionReceipt::L1Handler(L1HandlerTransactionReceipt {
                status: self.status.into(),
                block_hash: self.block_hash,
                block_number: self.block_number,
                events: self.transaction.output.events.clone(),
                messages_sent: self.transaction.output.messages_sent.clone(),
                transaction_hash: self.transaction.inner.hash(),
                actual_fee: self.transaction.execution_info.actual_fee.0.into(),
            }),
        }
    }
}

impl PendingTransaction {
    pub fn receipt(&self) -> RpcPendingTransactionReceipt {
        match &self.0.inner {
            Transaction::Invoke(_) => {
                RpcPendingTransactionReceipt::Invoke(PendingInvokeTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: self.0.inner.hash(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }

            Transaction::Declare(_) => {
                RpcPendingTransactionReceipt::Declare(PendingDeclareTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: self.0.inner.hash(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }

            Transaction::DeployAccount(_) => RpcPendingTransactionReceipt::DeployAccount(
                PendingDeployAccountTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: self.0.inner.hash(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                },
            ),

            Transaction::L1Handler(_) => RpcPendingTransactionReceipt::L1Handler(
                PendingL1HandlerTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: self.0.inner.hash(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                },
            ),
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
            KnownTransaction::Pending(tx) => api_to_rpc_transaction(tx.0.inner.clone().into()),
            KnownTransaction::Rejected(tx) => api_to_rpc_transaction(tx.transaction),
            KnownTransaction::Included(tx) => {
                api_to_rpc_transaction(tx.transaction.inner.clone().into())
            }
        }
    }
}

impl From<Transaction> for ApiTransaction {
    fn from(value: Transaction) -> Self {
        match value {
            Transaction::Invoke(tx) => ApiTransaction::Invoke(tx.0),
            Transaction::Declare(tx) => ApiTransaction::Declare(tx.inner),
            Transaction::DeployAccount(tx) => ApiTransaction::DeployAccount(tx.inner),
            Transaction::L1Handler(tx) => ApiTransaction::L1Handler(tx.0),
        }
    }
}

impl From<Transaction> for AccountTransaction {
    fn from(value: Transaction) -> Self {
        match value {
            Transaction::Invoke(tx) => AccountTransaction::Invoke(tx.0),
            Transaction::Declare(tx) => AccountTransaction::Declare(
                ExecutionDeclareTransaction::new(tx.inner, tx.compiled_class)
                    .expect("declare tx must have valid compiled class"),
            ),
            Transaction::DeployAccount(tx) => {
                AccountTransaction::DeployAccount(ExecutionDeployAccountTransaction {
                    tx: tx.inner,
                    contract_address: ContractAddress(patricia_key!(tx.contract_address)),
                })
            },
            Transaction::L1Handler(_) => panic!("L1HandlerTransaction is not an AccountTransaction"),
        }
    }
}
