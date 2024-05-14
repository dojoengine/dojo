use alloy_primitives::B256;

use crate::contract::ContractAddress;
use crate::fee::TxFeeInfo;
use crate::FieldElement;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Event {
    /// The contract address that emitted the event.
    pub from_address: ContractAddress,
    /// The event keys.
    pub keys: Vec<FieldElement>,
    /// The event data.
    pub data: Vec<FieldElement>,
}

/// Represents a message sent to L1.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MessageToL1 {
    /// The L2 contract address that sent the message.
    pub from_address: ContractAddress,
    /// The L1 contract address that the message is sent to.
    pub to_address: FieldElement,
    /// The payload of the message.
    pub payload: Vec<FieldElement>,
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
    pub execution_resources: TxExecutionResources,
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
    pub execution_resources: TxExecutionResources,
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
    pub execution_resources: TxExecutionResources,
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
    pub execution_resources: TxExecutionResources,
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
    pub fn resources_used(&self) -> &TxExecutionResources {
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

/// Transaction execution resources.
///
/// The resources consumed by a transaction during its execution.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TxExecutionResources {
    /// The number of cairo steps used
    pub steps: u64,
    /// The number of unused memory cells (each cell is roughly equivalent to a step)
    pub memory_holes: Option<u64>,
    /// The number of range_check builtin instances
    pub range_check_builtin: Option<u64>,
    /// The number of pedersen builtin instances
    pub pedersen_builtin: Option<u64>,
    /// The number of poseidon builtin instances
    pub poseidon_builtin: Option<u64>,
    /// The number of ec_op builtin instances
    pub ec_op_builtin: Option<u64>,
    /// The number of ecdsa builtin instances
    pub ecdsa_builtin: Option<u64>,
    /// The number of bitwise builtin instances
    pub bitwise_builtin: Option<u64>,
    /// The number of keccak builtin instances
    pub keccak_builtin: Option<u64>,

    pub segment_arena_builtin: Option<u64>,
}
