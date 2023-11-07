use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use starknet::core::types::{Event, FieldElement, MsgToL1};

use crate::contract::{ClassHash, CompiledClassHash, ContractAddress, Nonce};

/// The hash of a transaction.
pub type TxHash = FieldElement;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transaction {
    Invoke(InvokeTx),
    Declare(DeclareTx),
    L1Handler(L1HandlerTx),
    DeployAccount(DeployAccountTx),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvokeTx {
    V1(InvokeTxV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeTxV1 {
    pub transaction_hash: TxHash,
    pub nonce: Nonce,
    pub max_fee: u128,
    pub calldata: Vec<FieldElement>,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeclareTx {
    V1(DeclareTxV1),
    V2(DeclareTxV2),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclareTxV1 {
    pub transaction_hash: TxHash,
    pub max_fee: u128,
    pub nonce: Nonce,
    pub class_hash: ClassHash,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclareTxV2 {
    pub transaction_hash: TxHash,
    pub max_fee: u128,
    pub nonce: Nonce,
    pub class_hash: ClassHash,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
    pub compiled_class_hash: CompiledClassHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1HandlerTx {
    pub transaction_hash: TxHash,
    pub version: FieldElement,
    pub nonce: Nonce,
    pub paid_l1_fee: u128,
    pub calldata: Vec<FieldElement>,
    pub contract_address: ContractAddress,
    pub entry_point_selector: FieldElement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum FinalityStatus {
    AcceptedOnL2,
    AcceptedOnL1,
}

pub type ExecutionResources = HashMap<String, usize>;

/// The receipt of a transaction containing the outputs of its execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub actual_fee: u128,
    pub events: Vec<Event>,
    pub messages_sent: Vec<MsgToL1>,
    pub revert_error: Option<String>,
    pub actual_resources: ExecutionResources,
    pub contract_address: Option<ContractAddress>,
}
