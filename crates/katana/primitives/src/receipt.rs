use std::iter;

use alloy_primitives::B256;
use derive_more::{AsRef, Deref};
use starknet::core::utils::starknet_keccak;
use starknet_types_core::hash::{self, StarkHash};

use crate::contract::ContractAddress;
use crate::fee::TxFeeInfo;
use crate::trace::TxResources;
use crate::transaction::TxHash;
use crate::Felt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

#[derive(Debug, Clone, AsRef, Deref, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReceiptWithTxHash {
    /// The hash of the transaction.
    pub tx_hash: TxHash,
    /// The raw transaction.
    #[deref]
    #[as_ref]
    pub receipt: Receipt,
}

impl ReceiptWithTxHash {
    pub fn new(hash: TxHash, receipt: Receipt) -> Self {
        Self { tx_hash: hash, receipt }
    }

    /// Computes the hash of the receipt. This is used for computing the receipts commitment.
    ///
    /// See the Starknet [docs] for reference.
    ///
    /// [docs]: https://docs.starknet.io/architecture-and-concepts/network-architecture/block-structure/#receipt_hash
    pub fn compute_hash(&self) -> Felt {
        let messages_hash = self.compute_messages_to_l1_hash();
        let revert_reason_hash = if let Some(reason) = self.revert_reason() {
            starknet_keccak(reason.as_bytes())
        } else {
            Felt::ZERO
        };

        hash::Poseidon::hash_array(&[
            self.tx_hash,
            self.receipt.fee().overall_fee.into(),
            messages_hash,
            revert_reason_hash,
            Felt::ZERO, // L2 gas consumption.
            self.receipt.fee().gas_consumed.into(),
            // self.receipt.fee().l1_data_gas.into(),
        ])
    }

    // H(n, from, to, H(payload), ...), where n, is the total number of messages, the payload is
    // prefixed by its length, and h is the Poseidon hash function.
    fn compute_messages_to_l1_hash(&self) -> Felt {
        let messages = self.messages_sent();
        let messages_len = messages.len();

        // Allocate all the memory in advance; times 3 because [ from, to, h(payload) ]
        let mut accumulator: Vec<Felt> = Vec::with_capacity((messages_len * 3) + 1);
        accumulator.push(Felt::from(messages_len));

        let elements = messages.iter().fold(accumulator, |mut acc, msg| {
            // Compute the payload hash; h(n, payload_1, ..., payload_n)
            let len = Felt::from(msg.payload.len());
            let payload = iter::once(len).chain(msg.payload.clone()).collect::<Vec<Felt>>();
            let payload_hash = hash::Poseidon::hash_array(&payload);

            acc.push(msg.from_address.into());
            acc.push(msg.to_address);
            acc.push(payload_hash);

            acc
        });

        hash::Poseidon::hash_array(&elements)
    }
}
