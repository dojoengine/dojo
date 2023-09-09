use std::sync::Arc;

use blockifier::execution::contract_class::ContractClass;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction as ExecutionTransaction;
use blockifier::transaction::transactions::{
    DeclareTransaction as ExecutionDeclareTransaction,
    DeployAccountTransaction as ExecutionDeployAccountTransaction,
    L1HandlerTransaction as ExecutionL1HandlerTransaction,
};
use starknet::core::types::{
    DeclareTransactionReceipt, DeployAccountTransactionReceipt, Event, FieldElement,
    FlattenedSierraClass, InvokeTransactionReceipt, L1HandlerTransactionReceipt, MsgToL1,
    PendingDeclareTransactionReceipt, PendingDeployAccountTransactionReceipt,
    PendingInvokeTransactionReceipt, PendingL1HandlerTransactionReceipt,
    PendingTransactionReceipt as RpcPendingTransactionReceipt, Transaction as RpcTransaction,
    TransactionFinalityStatus, TransactionReceipt as RpcTransactionReceipt,
};
use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::transaction::{
    DeclareTransaction as ApiDeclareTransaction,
    DeployAccountTransaction as ApiDeployAccountTransaction, Fee,
    InvokeTransaction as ApiInvokeTransaction, L1HandlerTransaction as ApiL1HandlerTransaction,
    Transaction as ApiTransaction,
};

use crate::execution::ExecutedTransaction;
use crate::utils::transaction::api_to_rpc_transaction;

/// Represents all transactions that are known to the sequencer.
#[derive(Debug, Clone)]
pub enum KnownTransaction {
    Pending(PendingTransaction),
    Included(IncludedTransaction),
    Rejected(Arc<RejectedTransaction>),
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
    pub finality_status: TransactionFinalityStatus,
}

/// A transaction that is known to be rejected by the sequencer i.e.,
/// transaction that didn't pass the validation logic.
#[derive(Debug, Clone)]
pub struct RejectedTransaction {
    pub inner: Transaction,
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
            Transaction::L1Handler(tx) => tx.inner.transaction_hash.0.into(),
            Transaction::DeployAccount(tx) => tx.inner.transaction_hash.0.into(),
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct L1HandlerTransaction {
    pub inner: ApiL1HandlerTransaction,
    pub paid_l1_fee: u128,
}

impl IncludedTransaction {
    pub fn receipt(&self) -> RpcTransactionReceipt {
        match &self.transaction.inner {
            Transaction::Invoke(_) => RpcTransactionReceipt::Invoke(InvokeTransactionReceipt {
                block_hash: self.block_hash,
                block_number: self.block_number,
                finality_status: self.finality_status,
                events: self.transaction.output.events.clone(),
                execution_result: self.transaction.execution_result(),
                messages_sent: self.transaction.output.messages_sent.clone(),
                transaction_hash: self.transaction.inner.hash(),
                actual_fee: self.transaction.execution_info.actual_fee.0.into(),
            }),

            Transaction::Declare(_) => RpcTransactionReceipt::Declare(DeclareTransactionReceipt {
                block_hash: self.block_hash,
                block_number: self.block_number,
                finality_status: self.finality_status,
                events: self.transaction.output.events.clone(),
                transaction_hash: self.transaction.inner.hash(),
                execution_result: self.transaction.execution_result(),
                messages_sent: self.transaction.output.messages_sent.clone(),
                actual_fee: self.transaction.execution_info.actual_fee.0.into(),
            }),

            Transaction::DeployAccount(tx) => {
                RpcTransactionReceipt::DeployAccount(DeployAccountTransactionReceipt {
                    block_hash: self.block_hash,
                    block_number: self.block_number,
                    contract_address: tx.contract_address,
                    finality_status: self.finality_status,
                    events: self.transaction.output.events.clone(),
                    transaction_hash: self.transaction.inner.hash(),
                    execution_result: self.transaction.execution_result(),
                    messages_sent: self.transaction.output.messages_sent.clone(),
                    actual_fee: self.transaction.execution_info.actual_fee.0.into(),
                })
            }

            Transaction::L1Handler(_) => {
                RpcTransactionReceipt::L1Handler(L1HandlerTransactionReceipt {
                    block_hash: self.block_hash,
                    block_number: self.block_number,
                    finality_status: self.finality_status,
                    events: self.transaction.output.events.clone(),
                    transaction_hash: self.transaction.inner.hash(),
                    execution_result: self.transaction.execution_result(),
                    messages_sent: self.transaction.output.messages_sent.clone(),
                    actual_fee: self.transaction.execution_info.actual_fee.0.into(),
                })
            }
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
                    execution_result: self.0.execution_result(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }

            Transaction::Declare(_) => {
                RpcPendingTransactionReceipt::Declare(PendingDeclareTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: self.0.inner.hash(),
                    execution_result: self.0.execution_result(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }

            Transaction::DeployAccount(_) => RpcPendingTransactionReceipt::DeployAccount(
                PendingDeployAccountTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: self.0.inner.hash(),
                    execution_result: self.0.execution_result(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                },
            ),

            Transaction::L1Handler(_) => {
                RpcPendingTransactionReceipt::L1Handler(PendingL1HandlerTransactionReceipt {
                    events: self.0.output.events.clone(),
                    transaction_hash: self.0.inner.hash(),
                    execution_result: self.0.execution_result(),
                    messages_sent: self.0.output.messages_sent.clone(),
                    actual_fee: self.0.execution_info.actual_fee.0.into(),
                })
            }
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
        KnownTransaction::Rejected(Arc::new(transaction))
    }
}

impl From<KnownTransaction> for RpcTransaction {
    fn from(transaction: KnownTransaction) -> Self {
        match transaction {
            KnownTransaction::Pending(tx) => api_to_rpc_transaction(tx.0.inner.clone().into()),
            KnownTransaction::Rejected(tx) => api_to_rpc_transaction(tx.inner.clone().into()),
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
            Transaction::L1Handler(tx) => ApiTransaction::L1Handler(tx.inner),
            Transaction::DeployAccount(tx) => ApiTransaction::DeployAccount(tx.inner),
        }
    }
}

impl From<Transaction> for ExecutionTransaction {
    fn from(value: Transaction) -> Self {
        match value {
            Transaction::Invoke(tx) => {
                ExecutionTransaction::AccountTransaction(AccountTransaction::Invoke(tx.0))
            }

            Transaction::Declare(tx) => {
                ExecutionTransaction::AccountTransaction(AccountTransaction::Declare(
                    ExecutionDeclareTransaction::new(tx.inner, tx.compiled_class)
                        .expect("declare tx must have valid compiled class"),
                ))
            }

            Transaction::DeployAccount(tx) => ExecutionTransaction::AccountTransaction(
                AccountTransaction::DeployAccount(ExecutionDeployAccountTransaction {
                    tx: tx.inner,
                    contract_address: ContractAddress(patricia_key!(tx.contract_address)),
                }),
            ),

            Transaction::L1Handler(tx) => {
                ExecutionTransaction::L1HandlerTransaction(ExecutionL1HandlerTransaction {
                    tx: tx.inner,
                    paid_fee_on_l1: Fee(tx.paid_l1_fee),
                })
            }
        }
    }
}
