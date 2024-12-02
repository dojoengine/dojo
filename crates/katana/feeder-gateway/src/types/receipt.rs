use katana_primitives::receipt::{Event, MessageToL1};
use katana_primitives::Felt;
use serde::Deserialize;
use starknet::providers::sequencer::models::{
    ExecutionResources, L1ToL2Message, TransactionExecutionStatus,
};

#[derive(Debug, Deserialize)]
pub struct ConfirmedReceipt {
    pub transaction_hash: Felt,
    pub transaction_index: u64,
    #[serde(default)]
    pub execution_status: Option<TransactionExecutionStatus>,
    #[serde(default)]
    pub revert_error: Option<String>,
    #[serde(default)]
    pub execution_resources: Option<ExecutionResources>,
    pub l1_to_l2_consumed_message: Option<L1ToL2Message>,
    pub l2_to_l1_messages: Vec<MessageToL1>,
    pub events: Vec<Event>,
    pub actual_fee: Felt,
}
