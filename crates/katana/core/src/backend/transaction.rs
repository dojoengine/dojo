use std::collections::HashMap;
use std::vec;

use blockifier::execution::entry_point::CallInfo;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::TransactionExecutionInfo;
use starknet::core::types::TransactionStatus;
use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::core::{ContractAddress, EntryPointSelector};
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;
use starknet_api::transaction::{
    Calldata, DeclareTransactionOutput, DeployAccountTransactionOutput, DeployTransactionOutput,
    Event, Fee, InvokeTransactionOutput, L1HandlerTransactionOutput, MessageToL1, Transaction,
    TransactionHash, TransactionOutput, TransactionReceipt,
};

pub struct ExternalFunctionCall {
    pub calldata: Calldata,
    pub contract_address: ContractAddress,
    pub entry_point_selector: EntryPointSelector,
}

#[derive(Debug)]
pub struct StarknetTransaction {
    pub inner: Transaction,
    pub status: TransactionStatus,
    pub block_hash: Option<BlockHash>,
    pub block_number: Option<BlockNumber>,
    pub execution_info: Option<TransactionExecutionInfo>,
    pub execution_error: Option<TransactionExecutionError>,
}

impl StarknetTransaction {
    pub fn new(
        inner: Transaction,
        status: TransactionStatus,
        execution_info: Option<TransactionExecutionInfo>,
        execution_error: Option<TransactionExecutionError>,
    ) -> Self {
        // TODO: uncomment this once `Reverted` transaction error type is
        // `TransactionExecutionError`.
        //
        // if status == TransactionStatus::Rejected && execution_error.is_none() {
        //     panic!("rejected transaction must have an execution error");
        // };

        Self {
            inner,
            status,
            execution_info,
            execution_error,
            block_hash: None,
            block_number: None,
        }
    }

    pub fn actual_fee(&self) -> Fee {
        self.execution_info.as_ref().map_or(Fee(0), |info| info.actual_fee)
    }

    pub fn receipt(&self) -> TransactionReceipt {
        TransactionReceipt {
            output: self.output(),
            transaction_hash: self.inner.transaction_hash(),
            block_number: self.block_number.unwrap_or(BlockNumber(0)),
            block_hash: self.block_hash.unwrap_or(BlockHash(stark_felt!(0_u8))),
        }
    }

    pub fn emitted_events(&self) -> Vec<Event> {
        let mut events: Vec<Event> = vec![];

        fn get_events_recursively(call_info: &CallInfo) -> Vec<Event> {
            let mut events: Vec<Event> = vec![];

            events.extend(call_info.execution.events.iter().map(|e| Event {
                content: e.event.clone(),
                from_address: call_info.call.storage_address,
            }));

            call_info.inner_calls.iter().for_each(|call| {
                events.extend(get_events_recursively(call));
            });

            events
        }

        let Some(ref execution_info) = self.execution_info else {
            return events;
        };

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

    pub fn l2_to_l1_messages(&self) -> Vec<MessageToL1> {
        let mut messages: Vec<MessageToL1> = vec![];

        fn get_messages_recursively(info: &CallInfo) -> Vec<MessageToL1> {
            let mut messages: Vec<MessageToL1> = vec![];

            messages.extend(info.execution.l2_to_l1_messages.iter().map(|m| MessageToL1 {
                to_address: m.message.to_address,
                payload: m.message.payload.clone(),
                from_address: info.call.caller_address,
            }));

            info.inner_calls.iter().for_each(|call| {
                messages.extend(get_messages_recursively(call));
            });

            messages
        }

        let Some(ref execution_info) = self.execution_info else {
            return messages;
        };

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

    pub(crate) fn output(&self) -> TransactionOutput {
        let actual_fee = self.actual_fee();
        let events = self.emitted_events();
        let messages_sent = self.l2_to_l1_messages();

        match self.inner {
            Transaction::Invoke(_) => TransactionOutput::Invoke(InvokeTransactionOutput {
                events,
                actual_fee,
                messages_sent,
            }),
            Transaction::Declare(_) => TransactionOutput::Declare(DeclareTransactionOutput {
                events,
                actual_fee,
                messages_sent,
            }),
            Transaction::DeployAccount(_) => {
                TransactionOutput::DeployAccount(DeployAccountTransactionOutput {
                    events,
                    actual_fee,
                    messages_sent,
                })
            }
            Transaction::L1Handler(_) => TransactionOutput::L1Handler(L1HandlerTransactionOutput {
                events,
                actual_fee,
                messages_sent,
            }),
            Transaction::Deploy(_) => TransactionOutput::Deploy(DeployTransactionOutput {
                events,
                actual_fee,
                messages_sent,
            }),
        }
    }
}

#[derive(Debug, Default)]
pub struct StarknetTransactions {
    pub transactions: HashMap<TransactionHash, StarknetTransaction>,
}

impl StarknetTransactions {
    pub fn by_hash(&self, hash: &TransactionHash) -> Option<&StarknetTransaction> {
        self.transactions.get(hash)
    }

    pub fn total(&self) -> usize {
        self.transactions.len()
    }
}
