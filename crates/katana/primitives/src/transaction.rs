use alloy_primitives::B256;
use derive_more::{AsRef, Deref, From};
use starknet::core::types::{DataAvailabilityMode, ResourceBoundsMapping};

use crate::chain::ChainId;
use crate::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use crate::contract::{ContractAddress, Nonce};
use crate::utils::transaction::{
    compute_declare_v1_tx_hash, compute_declare_v2_tx_hash, compute_declare_v3_tx_hash,
    compute_deploy_account_v1_tx_hash, compute_deploy_account_v3_tx_hash,
    compute_invoke_v1_tx_hash, compute_l1_handler_tx_hash,
};
use crate::{utils, FieldElement};

/// The hash of a transaction.
pub type TxHash = FieldElement;
/// The sequential number for all the transactions.
pub type TxNumber = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tx {
    Invoke(InvokeTx),
    Declare(DeclareTx),
    L1Handler(L1HandlerTx),
    DeployAccount(DeployAccountTx),
}

pub enum TxRef<'a> {
    Invoke(&'a InvokeTx),
    Declare(&'a DeclareTx),
    L1Handler(&'a L1HandlerTx),
    DeployAccount(&'a DeployAccountTx),
}

impl<'a> From<TxRef<'a>> for Tx {
    fn from(value: TxRef<'a>) -> Self {
        match value {
            TxRef::Invoke(tx) => Tx::Invoke(tx.clone()),
            TxRef::Declare(tx) => Tx::Declare(tx.clone()),
            TxRef::L1Handler(tx) => Tx::L1Handler(tx.clone()),
            TxRef::DeployAccount(tx) => Tx::DeployAccount(tx.clone()),
        }
    }
}

/// Represents a transaction that has all the necessary data to be executed.
#[derive(Debug, Clone, From)]
pub enum ExecutableTx {
    Invoke(InvokeTx),
    L1Handler(L1HandlerTx),
    Declare(DeclareTxWithClass),
    DeployAccount(DeployAccountTx),
}

impl ExecutableTx {
    pub fn tx_ref(&self) -> TxRef<'_> {
        match self {
            ExecutableTx::Invoke(tx) => TxRef::Invoke(tx),
            ExecutableTx::L1Handler(tx) => TxRef::L1Handler(tx),
            ExecutableTx::Declare(tx) => TxRef::Declare(tx),
            ExecutableTx::DeployAccount(tx) => TxRef::DeployAccount(tx),
        }
    }
}

#[derive(Debug, Clone, AsRef, Deref)]
pub struct ExecutableTxWithHash {
    /// The hash of the transaction.
    pub hash: TxHash,
    /// The raw transaction.
    #[deref]
    #[as_ref]
    pub transaction: ExecutableTx,
}

impl ExecutableTxWithHash {
    pub fn new(transaction: ExecutableTx) -> Self {
        let hash = match &transaction {
            ExecutableTx::L1Handler(tx) => tx.calculate_hash(),
            ExecutableTx::Invoke(tx) => tx.calculate_hash(false),
            ExecutableTx::Declare(tx) => tx.calculate_hash(false),
            ExecutableTx::DeployAccount(tx) => tx.calculate_hash(false),
        };
        Self { hash, transaction }
    }

    pub fn new_query(transaction: ExecutableTx, is_query: bool) -> Self {
        let hash = match &transaction {
            ExecutableTx::L1Handler(tx) => tx.calculate_hash(),
            ExecutableTx::Invoke(tx) => tx.calculate_hash(is_query),
            ExecutableTx::Declare(tx) => tx.calculate_hash(is_query),
            ExecutableTx::DeployAccount(tx) => tx.calculate_hash(is_query),
        };
        Self { hash, transaction }
    }
}

#[derive(Debug, Clone, AsRef, Deref)]
pub struct DeclareTxWithClass {
    /// The Sierra class, if any.
    pub sierra_class: Option<FlattenedSierraClass>,
    /// The compiled contract class.
    pub compiled_class: CompiledClass,
    /// The raw transaction.
    #[deref]
    #[as_ref]
    pub transaction: DeclareTx,
}

impl DeclareTxWithClass {
    pub fn new_with_classes(
        transaction: DeclareTx,
        sierra_class: FlattenedSierraClass,
        compiled_class: CompiledClass,
    ) -> Self {
        Self { sierra_class: Some(sierra_class), compiled_class, transaction }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InvokeTx {
    V1(InvokeTxV1),
    V3(InvokeTxV3),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InvokeTxV1 {
    /// The chain id of the chain on which the transaction is initiated.
    ///
    /// Used as a simple replay attack protection.
    pub chain_id: ChainId,
    /// The account address which the transaction is initiated from.
    pub sender_address: ContractAddress,
    /// The nonce value of the account. Corresponds to the number of transactions initiated by
    /// sender.
    pub nonce: Nonce,
    /// The data used as the input to the execute entry point of sender account contract.
    pub calldata: Vec<FieldElement>,
    /// The transaction signature associated with the sender address.
    pub signature: Vec<FieldElement>,
    /// The max fee that the sender is willing to pay for the transaction.
    pub max_fee: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InvokeTxV3 {
    /// The chain id of the chain on which the transaction is initiated.
    ///
    /// Used as a simple replay attack protection.
    pub chain_id: ChainId,
    /// The account address which the transaction is initiated from.
    pub sender_address: ContractAddress,
    /// The nonce value of the account. Corresponds to the number of transactions initiated by
    /// sender.
    pub nonce: FieldElement,
    /// The data used as the input to the execute entry point of sender account contract.
    pub calldata: Vec<FieldElement>,
    /// The transaction signature associated with the sender address.
    pub signature: Vec<FieldElement>,
    /// Resource bounds for the transaction execution.
    pub resource_bounds: ResourceBoundsMapping,
    /// The tip for the transaction.
    pub tip: u64,
    /// Data needed to allow the paymaster to pay for the transaction in native tokens.
    pub paymaster_data: Vec<FieldElement>,
    /// Data needed to deploy the account contract from which this tx will be initiated. This field
    /// is used when the transaction is initiated from a address that is not yet deployed. The
    /// account contract will be deployed first before the function invocation is executed.
    ///
    /// The list contains the class_hash, salt, and the calldata needed for the constructor for
    /// account deployment.
    pub account_deployment_data: Vec<FieldElement>,
    /// The storage domain of the account's nonce (an account has a nonce per da mode)
    pub nonce_data_availability_mode: DataAvailabilityMode,
    /// The storage domain of the account's balance from which fee will be charged
    pub fee_data_availability_mode: DataAvailabilityMode,
}

impl InvokeTx {
    /// Compute the hash of the transaction.
    pub fn calculate_hash(&self, is_query: bool) -> TxHash {
        match self {
            InvokeTx::V1(tx) => compute_invoke_v1_tx_hash(
                tx.sender_address.into(),
                &tx.calldata,
                tx.max_fee,
                tx.chain_id.into(),
                tx.nonce,
                is_query,
            ),

            InvokeTx::V3(tx) => utils::transaction::compute_invoke_v3_tx_hash(
                tx.sender_address.into(),
                &tx.calldata,
                tx.tip,
                &tx.resource_bounds.l1_gas,
                &tx.resource_bounds.l2_gas,
                &tx.paymaster_data,
                tx.chain_id.into(),
                tx.nonce,
                &tx.nonce_data_availability_mode,
                &tx.fee_data_availability_mode,
                &tx.account_deployment_data,
                is_query,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeclareTx {
    V1(DeclareTxV1),
    V2(DeclareTxV2),
    V3(DeclareTxV3),
}

impl DeclareTx {
    pub fn class_hash(&self) -> ClassHash {
        match self {
            DeclareTx::V1(tx) => tx.class_hash,
            DeclareTx::V2(tx) => tx.class_hash,
            DeclareTx::V3(tx) => tx.class_hash,
        }
    }
}

/// Represents a declare transaction type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxV1 {
    /// The chain id of the chain on which the transaction is initiated.
    ///
    /// Used as a simple replay attack protection.
    pub chain_id: ChainId,
    /// The account address which the transaction is initiated from.
    pub sender_address: ContractAddress,
    /// The nonce value of the account. Corresponds to the number of transactions initiated by
    /// sender.
    pub nonce: FieldElement,
    /// The transaction signature associated with the sender address.
    pub signature: Vec<FieldElement>,
    /// The class hash of the contract class to be declared.
    pub class_hash: ClassHash,
    /// The max fee that the sender is willing to pay for the transaction.
    pub max_fee: u128,
}

/// Represents a declare transaction type.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxV2 {
    /// The chain id of the chain on which the transaction is initiated.
    ///
    /// Used as a simple replay attack protection.
    pub chain_id: ChainId,
    /// The account address which the transaction is initiated from.
    pub sender_address: ContractAddress,
    /// The nonce value of the account. Corresponds to the number of transactions initiated by
    /// sender.
    pub nonce: FieldElement,
    /// The transaction signature associated with the sender address.
    pub signature: Vec<FieldElement>,
    /// The class hash of the contract class to be declared.
    pub class_hash: ClassHash,
    /// The compiled class hash of the contract class (only if it's a Sierra class).
    pub compiled_class_hash: CompiledClassHash,
    /// The max fee that the sender is willing to pay for the transaction.
    pub max_fee: u128,
}

/// Represents a declare transaction type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxV3 {
    /// The chain id of the chain on which the transaction is initiated.
    ///
    /// Used as a simple replay attack protection.
    pub chain_id: ChainId,
    /// The account address which the transaction is initiated from.
    pub sender_address: ContractAddress,
    /// The nonce value of the account. Corresponds to the number of transactions initiated by
    /// sender.
    pub nonce: FieldElement,
    /// The transaction signature associated with the sender address.
    pub signature: Vec<FieldElement>,
    /// The class hash of the contract class to be declared.
    pub class_hash: ClassHash,
    /// The CASM class hash of the Sierra contract class.
    pub compiled_class_hash: CompiledClassHash,
    /// Resource bounds for the transaction execution
    pub resource_bounds: ResourceBoundsMapping,
    /// The tip for the transaction
    pub tip: u64,
    /// Data needed to allow the paymaster to pay for the transaction in native tokens
    pub paymaster_data: Vec<FieldElement>,
    /// Data needed to deploy the account contract from which this tx will be initiated
    pub account_deployment_data: Vec<FieldElement>,
    /// The storage domain of the account's nonce (an account has a nonce per da mode)
    pub nonce_data_availability_mode: DataAvailabilityMode,
    /// The storage domain of the account's balance from which fee will be charged
    pub fee_data_availability_mode: DataAvailabilityMode,
}

impl DeclareTx {
    /// Compute the hash of the transaction.
    pub fn calculate_hash(&self, is_query: bool) -> TxHash {
        match self {
            DeclareTx::V1(tx) => compute_declare_v1_tx_hash(
                tx.sender_address.into(),
                tx.class_hash,
                tx.max_fee,
                tx.chain_id.into(),
                tx.nonce,
                is_query,
            ),

            DeclareTx::V2(tx) => compute_declare_v2_tx_hash(
                tx.sender_address.into(),
                tx.class_hash,
                tx.max_fee,
                tx.chain_id.into(),
                tx.nonce,
                tx.compiled_class_hash,
                is_query,
            ),

            DeclareTx::V3(tx) => compute_declare_v3_tx_hash(
                tx.sender_address.into(),
                tx.class_hash,
                tx.compiled_class_hash,
                tx.tip,
                &tx.resource_bounds.l1_gas,
                &tx.resource_bounds.l2_gas,
                &tx.paymaster_data,
                tx.chain_id.into(),
                tx.nonce,
                &tx.nonce_data_availability_mode,
                &tx.fee_data_availability_mode,
                &tx.account_deployment_data,
                is_query,
            ),
        }
    }
}

/// The transaction type for L1 handler invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct L1HandlerTx {
    /// The L1 to L2 message nonce.
    pub nonce: Nonce,
    /// The chain id.
    pub chain_id: ChainId,
    /// Amount of fee paid on L1.
    pub paid_fee_on_l1: u128,
    /// Transaction version.
    pub version: FieldElement,
    /// L1 to L2 message hash.
    pub message_hash: B256,
    /// The input to the L1 handler function.
    pub calldata: Vec<FieldElement>,
    /// Contract address of the L1 handler.
    pub contract_address: ContractAddress,
    /// The L1 handler function selector.
    pub entry_point_selector: FieldElement,
}

impl L1HandlerTx {
    /// Compute the hash of the transaction.
    pub fn calculate_hash(&self) -> TxHash {
        compute_l1_handler_tx_hash(
            self.version,
            self.contract_address.into(),
            self.entry_point_selector,
            &self.calldata,
            self.chain_id.into(),
            self.nonce,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeployAccountTx {
    V1(DeployAccountTxV1),
    V3(DeployAccountTxV3),
}

impl DeployAccountTx {
    pub fn contract_address(&self) -> ContractAddress {
        match self {
            DeployAccountTx::V1(tx) => tx.contract_address,
            DeployAccountTx::V3(tx) => tx.contract_address,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeployAccountTxV1 {
    /// The chain id of the chain on which the transaction is initiated.
    ///
    /// Used as a simple replay attack protection.
    pub chain_id: ChainId,
    /// The nonce value of the account. Corresponds to the number of transactions initiated by
    /// sender.
    pub nonce: Nonce,
    /// The transaction signature associated with the sender address.
    pub signature: Vec<FieldElement>,
    /// The hash of the contract class from which the account contract will be deployed from.
    pub class_hash: ClassHash,
    /// The contract address of the account contract that will be deployed.
    pub contract_address: ContractAddress,
    /// The salt used to generate the contract address.
    pub contract_address_salt: FieldElement,
    /// The input data to the constructor function of the contract class.
    pub constructor_calldata: Vec<FieldElement>,
    /// The max fee that the sender is willing to pay for the transaction.
    pub max_fee: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeployAccountTxV3 {
    /// The chain id of the chain on which the transaction is initiated.
    ///
    /// Used as a simple replay attack protection.
    pub chain_id: ChainId,
    /// The nonce value of the account. Corresponds to the number of transactions initiated by
    /// sender.
    pub nonce: Nonce,
    /// The transaction signature associated with the sender address.
    pub signature: Vec<FieldElement>,
    /// The hash of the contract class from which the account contract will be deployed from.
    pub class_hash: ClassHash,
    /// The contract address of the account contract that will be deployed.
    pub contract_address: ContractAddress,
    /// The salt used to generate the contract address.
    pub contract_address_salt: FieldElement,
    /// The input data to the constructor function of the contract class.
    pub constructor_calldata: Vec<FieldElement>,
    /// Resource bounds for the transaction execution
    pub resource_bounds: ResourceBoundsMapping,
    /// The tip for the transaction
    pub tip: u64,
    /// Data needed to allow the paymaster to pay for the transaction in native tokens
    pub paymaster_data: Vec<FieldElement>,
    /// The storage domain of the account's nonce (an account has a nonce per da mode)
    pub nonce_data_availability_mode: DataAvailabilityMode,
    /// The storage domain of the account's balance from which fee will be charged
    pub fee_data_availability_mode: DataAvailabilityMode,
}

impl DeployAccountTx {
    /// Compute the hash of the transaction.
    pub fn calculate_hash(&self, is_query: bool) -> TxHash {
        match self {
            DeployAccountTx::V1(tx) => compute_deploy_account_v1_tx_hash(
                tx.contract_address.into(),
                &tx.constructor_calldata,
                tx.class_hash,
                tx.contract_address_salt,
                tx.max_fee,
                tx.chain_id.into(),
                tx.nonce,
                is_query,
            ),

            DeployAccountTx::V3(tx) => compute_deploy_account_v3_tx_hash(
                tx.contract_address.into(),
                &tx.constructor_calldata,
                tx.class_hash,
                tx.contract_address_salt,
                tx.tip,
                &tx.resource_bounds.l1_gas,
                &tx.resource_bounds.l2_gas,
                &tx.paymaster_data,
                tx.chain_id.into(),
                tx.nonce,
                &tx.nonce_data_availability_mode,
                &tx.fee_data_availability_mode,
                is_query,
            ),
        }
    }
}

#[derive(Debug, Clone, AsRef, Deref, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TxWithHash {
    /// The hash of the transaction.
    pub hash: TxHash,
    /// The raw transaction.
    #[deref]
    #[as_ref]
    pub transaction: Tx,
}

impl From<ExecutableTxWithHash> for TxWithHash {
    fn from(tx: ExecutableTxWithHash) -> Self {
        Self { hash: tx.hash, transaction: tx.tx_ref().into() }
    }
}

impl From<&ExecutableTxWithHash> for TxWithHash {
    fn from(tx: &ExecutableTxWithHash) -> Self {
        Self { hash: tx.hash, transaction: tx.tx_ref().into() }
    }
}
