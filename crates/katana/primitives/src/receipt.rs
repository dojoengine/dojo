use alloy_primitives::B256;

use crate::contract::ContractAddress;
use crate::fee::TxFeeInfo;
use crate::trace::TxResources;
use crate::Felt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Event {
    /// The contract address that emitted the event.
    pub from_address: ContractAddress,
    /// The event keys.
    pub keys: Vec<Felt>,
    /// The event data.
    pub data: Vec<Felt>,
}

/// Represents a message sent to L1.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MessageToL1 {
    /// The L2 contract address that sent the message.
    pub from_address: ContractAddress,
    /// The L1 contract address that the message is sent to.
    pub to_address: Felt,
    /// The payload of the message.
    pub payload: Vec<Felt>,
}

/// Receipt for a `Invoke` transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InvokeTxReceipt {
    /// Information about the transaction fee.
    pub fee: TxFeeInfo,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// Messages sent to L1.
    pub messages_sent: Vec<MessageToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: TxResources,
}

/// Receipt for a `Declare` transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxReceipt {
    /// Information about the transaction fee.
    pub fee: TxFeeInfo,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// Messages sent to L1.
    pub messages_sent: Vec<MessageToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: TxResources,
}

/// Receipt for a `L1Handler` transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct L1HandlerTxReceipt {
    /// Information about the transaction fee.
    pub fee: TxFeeInfo,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// The hash of the L1 message
    pub message_hash: B256,
    /// Messages sent to L1.
    pub messages_sent: Vec<MessageToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: TxResources,
}

/// Receipt for a `DeployAccount` transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeployAccountTxReceipt {
    /// Information about the transaction fee.
    pub fee: TxFeeInfo,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// Messages sent to L1.
    pub messages_sent: Vec<MessageToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub execution_resources: TxResources,
    /// Contract address of the deployed account contract.
    pub contract_address: ContractAddress,
}

/// The receipt of a transaction containing the outputs of its execution.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        self.revert_reason().is_some()
    }

    /// Returns the revert reason if the transaction is reverted.
    pub fn revert_reason(&self) -> Option<&str> {
        match self {
            Receipt::Invoke(rct) => rct.revert_error.as_deref(),
            Receipt::Declare(rct) => rct.revert_error.as_deref(),
            Receipt::L1Handler(rct) => rct.revert_error.as_deref(),
            Receipt::DeployAccount(rct) => rct.revert_error.as_deref(),
        }
    }

    /// Returns the L1 messages sent.
    pub fn messages_sent(&self) -> &[MessageToL1] {
        match self {
            Receipt::Invoke(rct) => &rct.messages_sent,
            Receipt::Declare(rct) => &rct.messages_sent,
            Receipt::L1Handler(rct) => &rct.messages_sent,
            Receipt::DeployAccount(rct) => &rct.messages_sent,
        }
    }

    /// Returns the events emitted.
    pub fn events(&self) -> &[Event] {
        match self {
            Receipt::Invoke(rct) => &rct.events,
            Receipt::Declare(rct) => &rct.events,
            Receipt::L1Handler(rct) => &rct.events,
            Receipt::DeployAccount(rct) => &rct.events,
        }
    }

    /// Returns the execution resources used.
    pub fn resources_used(&self) -> &TxResources {
        match self {
            Receipt::Invoke(rct) => &rct.execution_resources,
            Receipt::Declare(rct) => &rct.execution_resources,
            Receipt::L1Handler(rct) => &rct.execution_resources,
            Receipt::DeployAccount(rct) => &rct.execution_resources,
        }
    }

    pub fn fee(&self) -> &TxFeeInfo {
        match self {
            Receipt::Invoke(rct) => &rct.fee,
            Receipt::Declare(rct) => &rct.fee,
            Receipt::L1Handler(rct) => &rct.fee,
            Receipt::DeployAccount(rct) => &rct.fee,
        }
    }
}
