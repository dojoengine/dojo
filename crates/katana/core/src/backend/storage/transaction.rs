use std::sync::Arc;

use blockifier::transaction::errors::TransactionExecutionError;
use starknet::core::types::{Event, FieldElement, MsgToL1};
use starknet_api::transaction::Transaction;

use crate::backend::pending::ExecutedTransaction;

/// The status of the included transactions.
#[derive(Debug)]
pub enum IncludedTransactionStatus {
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
#[derive(Debug)]
pub enum KnownTransaction {
    Pending(PendingTransaction),
    Included(IncludedTransaction),
    Rejected(RejectedTransaction),
}

#[derive(Debug)]
pub struct PendingTransaction(pub Arc<ExecutedTransaction>);

/// A transaction that is known to be included in a block. Which also includes
/// reverted transactions and transactions that are currently in the `pending` block.
#[derive(Debug)]
pub struct IncludedTransaction {
    pub block_number: u64,
    pub block_hash: FieldElement,
    // pub transaction: Transaction,
    // pub output: TransactionOutput,
    // pub execution_info: TransactionExecutionInfo,
    pub transaction: Arc<ExecutedTransaction>,
    pub status: IncludedTransactionStatus,
}

/// A transaction that is known to be rejected by the sequencer i.e.,
/// transaction that didn't pass the validation logic.
#[derive(Debug)]
pub struct RejectedTransaction {
    pub transaction: Transaction,
    pub execution_error: TransactionExecutionError,
}

#[derive(Debug, Clone)]
pub struct TransactionOutput {
    pub actual_fee: u128,
    pub events: Vec<Event>,
    pub messages_sent: Vec<MsgToL1>,
}
