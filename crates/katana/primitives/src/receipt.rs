use starknet::core::types::{Event, ExecutionResources, Hash256, MsgToL1};

use crate::contract::ContractAddress;

/// Receipt for a `Invoke` transaction.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InvokeTxReceipt {
    /// Actual fee paid for the transaction.
    pub actual_fee: u128,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// Messages sent to L1.
    pub messages_sent: Vec<MsgToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: ExecutionResources,
}

/// Receipt for a `Declare` transaction.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxReceipt {
    /// Actual fee paid for the transaction.
    pub actual_fee: u128,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// Messages sent to L1.
    pub messages_sent: Vec<MsgToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: ExecutionResources,
}

/// Receipt for a `L1Handler` transaction.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct L1HandlerTxReceipt {
    /// Actual fee paid for the transaction.
    pub actual_fee: u128,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// The hash of the L1 message
    pub message_hash: Hash256,
    /// Messages sent to L1.
    pub messages_sent: Vec<MsgToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: ExecutionResources,
}

/// Receipt for a `DeployAccount` transaction.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeployAccountTxReceipt {
    /// Actual fee paid for the transaction.
    pub actual_fee: u128,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// Messages sent to L1.
    pub messages_sent: Vec<MsgToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: ExecutionResources,
    /// Contract address of the deployed account contract.
    pub contract_address: ContractAddress,
}

/// The receipt of a transaction containing the outputs of its execution.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Receipt {
    Invoke(InvokeTxReceipt),
    Declare(DeclareTxReceipt),
    L1Handler(L1HandlerTxReceipt),
    DeployAccount(DeployAccountTxReceipt),
}

impl Receipt {
    /// Returns `true` if the transaction is reverted.
    ///
    /// A transaction is reverted if the `revert_error` field in the receipt is not `None`.
    pub fn is_reverted(&self) -> bool {
        match self {
            Receipt::Invoke(rct) => rct.revert_error.is_some(),
            Receipt::Declare(rct) => rct.revert_error.is_some(),
            Receipt::L1Handler(rct) => rct.revert_error.is_some(),
            Receipt::DeployAccount(rct) => rct.revert_error.is_some(),
        }
    }

    pub fn messages_sent(&self) -> &[MsgToL1] {
        match self {
            Receipt::Invoke(rct) => &rct.messages_sent,
            Receipt::Declare(rct) => &rct.messages_sent,
            Receipt::L1Handler(rct) => &rct.messages_sent,
            Receipt::DeployAccount(rct) => &rct.messages_sent,
        }
    }

    pub fn events(&self) -> &[Event] {
        match self {
            Receipt::Invoke(rct) => &rct.events,
            Receipt::Declare(rct) => &rct.events,
            Receipt::L1Handler(rct) => &rct.events,
            Receipt::DeployAccount(rct) => &rct.events,
        }
    }
}
