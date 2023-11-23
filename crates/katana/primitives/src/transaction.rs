use std::collections::HashMap;

use starknet::core::types::{Event, MsgToL1};

use crate::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, Nonce,
};
use crate::FieldElement;

/// The hash of a transaction.
pub type TxHash = FieldElement;
/// The sequential number for all the transactions..
pub type TxNumber = u64;

/// Represents a transaction that can be executed.
#[derive(Debug, Clone)]
pub enum ExecutionTx {
    Invoke(InvokeTx),
    L1Handler(L1HandlerTx),
    Declare(DeclareTxWithCompiledClass),
    DeployAccount(DeployAccountTxWithContractAddress),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Transaction {
    Invoke(InvokeTx),
    Declare(DeclareTx),
    L1Handler(L1HandlerTx),
    DeployAccount(DeployAccountTx),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InvokeTx {
    V1(InvokeTxV1),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InvokeTxV1 {
    pub transaction_hash: TxHash,
    pub nonce: Nonce,
    pub max_fee: u128,
    pub calldata: Vec<FieldElement>,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
}

#[derive(Debug, Clone)]
pub struct DeclareTxWithCompiledClass(pub DeclareTx, pub CompiledContractClass);

#[derive(Debug, Clone)]
pub struct DeployAccountTxWithContractAddress(pub DeployAccountTx, pub ContractAddress);

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeclareTx {
    V1(DeclareTxV1),
    V2(DeclareTxV2),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxV1 {
    pub transaction_hash: TxHash,
    pub max_fee: u128,
    pub nonce: Nonce,
    pub class_hash: ClassHash,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxV2 {
    pub transaction_hash: TxHash,
    pub max_fee: u128,
    pub nonce: Nonce,
    pub class_hash: ClassHash,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
    pub compiled_class_hash: CompiledClassHash,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct L1HandlerTx {
    pub transaction_hash: TxHash,
    pub version: FieldElement,
    pub nonce: Nonce,
    pub calldata: Vec<FieldElement>,
    pub contract_address: ContractAddress,
    pub entry_point_selector: FieldElement,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeployAccountTx {
    pub transaction_hash: TxHash,
    pub max_fee: u128,
    pub nonce: Nonce,
    pub class_hash: ClassHash,
    pub version: FieldElement,
    pub signature: Vec<FieldElement>,
    pub contract_address_salt: FieldElement,
    pub constructor_calldata: Vec<FieldElement>,
}

/// A transaction finality status.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FinalityStatus {
    AcceptedOnL2,
    AcceptedOnL1,
}

pub type ExecutionResources = HashMap<String, usize>;

/// The receipt of a transaction containing the outputs of its execution.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Receipt {
    /// Actual fee paid for the transaction.
    pub actual_fee: u128,
    /// Events emitted by contracts.
    pub events: Vec<Event>,
    /// Messages sent to L1.
    pub messages_sent: Vec<MsgToL1>,
    /// Revert error message if the transaction execution failed.
    pub revert_error: Option<String>,
    /// The execution resources used by the transaction.
    pub actual_resources: ExecutionResources,
    /// Contract address if the transaction deployed a contract. (only for deploy account tx)
    pub contract_address: Option<ContractAddress>,
}
