use derive_more::{AsRef, Deref};
use ethers::types::H256;

use crate::chain::ChainId;
use crate::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, FlattenedSierraClass, Nonce,
};
use crate::utils::transaction::{
    compute_declare_v1_tx_hash, compute_declare_v2_tx_hash, compute_deploy_account_v1_tx_hash,
    compute_invoke_v1_tx_hash, compute_l1_handler_tx_hash,
};
use crate::FieldElement;

/// The hash of a transaction.
pub type TxHash = FieldElement;
/// The sequential number for all the transactions..
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
#[derive(Debug, Clone)]
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

    pub fn new_query(transaction: ExecutableTx) -> Self {
        let hash = match &transaction {
            ExecutableTx::L1Handler(tx) => tx.calculate_hash(),
            ExecutableTx::Invoke(tx) => tx.calculate_hash(true),
            ExecutableTx::Declare(tx) => tx.calculate_hash(true),
            ExecutableTx::DeployAccount(tx) => tx.calculate_hash(true),
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InvokeTx {
    pub nonce: Nonce,
    pub max_fee: u128,
    pub chain_id: ChainId,
    pub version: FieldElement,
    pub calldata: Vec<FieldElement>,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
}

impl InvokeTx {
    /// Compute the hash of the transaction.
    pub fn calculate_hash(&self, is_query: bool) -> TxHash {
        compute_invoke_v1_tx_hash(
            self.sender_address.into(),
            &self.calldata,
            self.max_fee,
            self.chain_id.into(),
            self.nonce,
            is_query,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeclareTx {
    V1(DeclareTxV1),
    V2(DeclareTxV2),
}

impl DeclareTx {
    pub fn class_hash(&self) -> ClassHash {
        match self {
            DeclareTx::V1(tx) => tx.class_hash,
            DeclareTx::V2(tx) => tx.class_hash,
        }
    }
}

/// Represents a declare transaction type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxV1 {
    pub nonce: Nonce,
    pub max_fee: u128,
    pub chain_id: ChainId,
    /// The class hash of the contract class to be declared.
    pub class_hash: ClassHash,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
}

/// Represents a declare transaction type.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeclareTxV2 {
    pub nonce: Nonce,
    pub max_fee: u128,
    pub chain_id: ChainId,
    /// The class hash of the contract class to be declared.
    pub class_hash: ClassHash,
    pub signature: Vec<FieldElement>,
    pub sender_address: ContractAddress,
    /// The compiled class hash of the contract class (only if it's a Sierra class).
    pub compiled_class_hash: CompiledClassHash,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct L1HandlerTx {
    pub nonce: Nonce,
    pub chain_id: ChainId,
    pub paid_fee_on_l1: u128,
    pub version: FieldElement,
    pub message_hash: H256,
    pub calldata: Vec<FieldElement>,
    pub contract_address: ContractAddress,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeployAccountTx {
    pub nonce: Nonce,
    pub max_fee: u128,
    pub chain_id: ChainId,
    pub version: FieldElement,
    pub class_hash: ClassHash,
    pub signature: Vec<FieldElement>,
    pub contract_address: ContractAddress,
    pub contract_address_salt: FieldElement,
    pub constructor_calldata: Vec<FieldElement>,
}

impl DeployAccountTx {
    /// Compute the hash of the transaction.
    pub fn calculate_hash(&self, is_query: bool) -> TxHash {
        compute_deploy_account_v1_tx_hash(
            self.contract_address.into(),
            self.constructor_calldata.as_slice(),
            self.class_hash,
            self.contract_address_salt,
            self.max_fee,
            self.chain_id.into(),
            self.nonce,
            is_query,
        )
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

impl From<L1HandlerTx> for ExecutableTx {
    fn from(tx: L1HandlerTx) -> Self {
        ExecutableTx::L1Handler(tx)
    }
}

impl From<DeclareTxWithClass> for ExecutableTx {
    fn from(tx: DeclareTxWithClass) -> Self {
        ExecutableTx::Declare(tx)
    }
}

impl From<InvokeTx> for ExecutableTx {
    fn from(tx: InvokeTx) -> Self {
        ExecutableTx::Invoke(tx)
    }
}

impl From<DeployAccountTx> for ExecutableTx {
    fn from(tx: DeployAccountTx) -> Self {
        ExecutableTx::DeployAccount(tx)
    }
}
