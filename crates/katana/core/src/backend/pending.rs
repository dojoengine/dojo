use std::sync::Arc;

use blockifier::execution::entry_point::CallInfo;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::TransactionExecutionInfo;
use blockifier::transaction::transaction_execution::Transaction as ExecutionTransaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use blockifier::{block_context::BlockContext, state::cached_state::CachedState};
use starknet::core::types::{Event, FieldElement, MsgToL1};
use starknet_api::transaction::Transaction;
use tokio::sync::RwLock;
use tracing::warn;

use crate::util::convert_blockifier_tx_to_starknet_api_tx;

use super::storage::block::{Block, PartialHeader};
use super::storage::transaction::{
    KnownTransaction, PendingTransaction, RejectedTransaction, TransactionOutput,
};
use super::{state::MemDb, storage::BlockchainStorage};

#[derive(Debug)]
pub struct PendingBlock<'a> {
    /// The state of the pending block. It is the state that the
    /// transaction included in the pending block will be executed on.
    /// The changes made after the execution of a transaction will be
    /// persisted for the next included transaction.
    pub state: CachedState<MemDb>,
    pub block_context: &'a BlockContext,
    pub storage: &'a RwLock<BlockchainStorage>,
    pub transactions: Vec<Arc<ExecutedTransaction>>,
    pub outputs: Vec<TransactionOutput>,
}

impl<'a> PendingBlock<'a> {
    pub fn new(
        state: MemDb,
        block_context: &'a BlockContext,
        storage: &'a RwLock<BlockchainStorage>,
    ) -> Self {
        Self {
            storage,
            block_context,
            outputs: Vec::new(),
            transactions: Vec::new(),
            state: CachedState::new(state),
        }
    }

    /// Generate a new valid block which will be included to the blockchain.
    pub async fn generate_block(&self) -> Block {
        let parent_hash = self.storage.read().await.latest_hash;
        let partial_header = PartialHeader {
            // use the current latest hash as the parent hash for this new block
            parent_hash,
            gas_price: self.block_context.gas_price,
            number: self.block_context.block_number.0,
            timestamp: self.block_context.block_timestamp.0,
            sequencer_address: (*self.block_context.sequencer_address.0.key()).into(),
        };

        Block::new(partial_header, self.transactions.clone(), self.outputs.clone())
    }

    /// Reset the pending block. This will clear all the transactions and
    /// the state of the pending block.
    pub fn reset(&mut self, state: MemDb) {
        self.state = CachedState::new(state);
        self.transactions.clear();
        self.outputs.clear();
    }

    // Add a transaction to the pending block. The transaction will be executed
    // on the pending state. The transaction will be added to the pending block
    // if it passes the validation logic. Otherwise, the transaction will be
    // rejected. On both cases, the transaction will still be stored in the
    // storage.
    pub async fn add_transaction(&mut self, transaction: ExecutionTransaction) {
        let api_tx = convert_blockifier_tx_to_starknet_api_tx(&transaction);
        let res = execute_transaction(transaction, &mut self.state, self.block_context);

        let hash: FieldElement = api_tx.transaction_hash().0.into();

        let tx = match res {
            Ok(execution_info) => {
                let executed_tx = Arc::new(ExecutedTransaction::new(api_tx, execution_info));

                self.transactions.push(executed_tx.clone());
                self.outputs.push(executed_tx.output.clone());

                KnownTransaction::Pending(PendingTransaction(executed_tx))
            }

            Err(execution_error) => KnownTransaction::Rejected(RejectedTransaction {
                execution_error,
                transaction: api_tx,
            }),
        };

        self.storage.write().await.transactions.insert(hash, tx);
    }
}

#[derive(Debug)]
pub struct ExecutedTransaction {
    pub transaction: Transaction,
    pub output: TransactionOutput,
    pub execution_info: TransactionExecutionInfo,
}

impl ExecutedTransaction {
    pub fn new(transaction: Transaction, execution_info: TransactionExecutionInfo) -> Self {
        let actual_fee = execution_info.actual_fee.0;
        let events = Self::events(&execution_info);
        let messages_sent = Self::l2_to_l1_messages(&execution_info);

        Self {
            transaction,
            execution_info,
            output: TransactionOutput { actual_fee, events, messages_sent },
        }
    }

    fn events(execution_info: &TransactionExecutionInfo) -> Vec<Event> {
        let mut events: Vec<Event> = vec![];

        fn get_events_recursively(call_info: &CallInfo) -> Vec<Event> {
            let mut events: Vec<Event> = vec![];

            events.extend(call_info.execution.events.iter().map(|e| Event {
                from_address: (*call_info.call.storage_address.0.key()).into(),
                data: e.event.data.0.iter().map(|d| (*d).into()).collect(),
                keys: e.event.keys.iter().map(|k| k.0.into()).collect(),
            }));

            call_info.inner_calls.iter().for_each(|call| {
                events.extend(get_events_recursively(call));
            });

            events
        }

        if let Some(ref call) = execution_info.validate_call_info {
            events.extend(get_events_recursively(call));
        }

        if let Some(ref call) = execution_info.execute_call_info {
            events.extend(get_events_recursively(call));
        }

        if let Some(ref call) = execution_info.fee_transfer_call_info {
            events.extend(get_events_recursively(call));
        }

        events
    }

    fn l2_to_l1_messages(execution_info: &TransactionExecutionInfo) -> Vec<MsgToL1> {
        let mut messages = vec![];

        fn get_messages_recursively(info: &CallInfo) -> Vec<MsgToL1> {
            let mut messages = vec![];

            messages.extend(info.execution.l2_to_l1_messages.iter().map(|m| MsgToL1 {
                to_address:
                    FieldElement::from_byte_slice_be(m.message.to_address.0.as_bytes()).unwrap(),
                from_address: (*info.call.caller_address.0.key()).into(),
                payload: m.message.payload.0.iter().map(|p| (*p).into()).collect(),
            }));

            info.inner_calls.iter().for_each(|call| {
                messages.extend(get_messages_recursively(call));
            });

            messages
        }

        if let Some(ref info) = execution_info.validate_call_info {
            messages.extend(get_messages_recursively(info));
        }

        if let Some(ref info) = execution_info.execute_call_info {
            messages.extend(get_messages_recursively(info));
        }

        if let Some(ref info) = execution_info.fee_transfer_call_info {
            messages.extend(get_messages_recursively(info));
        }

        messages
    }
}

fn execute_transaction(
    transaction: ExecutionTransaction,
    pending_state: &mut CachedState<MemDb>,
    block_context: &BlockContext,
) -> Result<TransactionExecutionInfo, TransactionExecutionError> {
    let res = match transaction {
        ExecutionTransaction::AccountTransaction(tx) => tx.execute(pending_state, block_context),
        ExecutionTransaction::L1HandlerTransaction(tx) => tx.execute(pending_state, block_context),
    };

    match res {
        Ok(exec_info) => {
            if let Some(err) = &exec_info.revert_error {
                let formatted_err = format!("{:?}", err).replace("\\n", "\n");
                warn!("Transaction execution error: {formatted_err}");
            }
            Ok(exec_info)
        }
        Err(err) => {
            warn!("Transaction validation error: {err:?}");
            Err(err)
        }
    }
}
