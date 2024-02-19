use std::sync::Arc;

use anyhow::Result;
use derive_more::Deref;
use katana_primitives::chain::ChainId;
use katana_primitives::contract::{ClassHash, ContractAddress};
use katana_primitives::conversion::rpc::{
    compiled_class_hash_from_flattened_sierra_class, flattened_sierra_to_compiled_class,
    legacy_rpc_to_compiled_class,
};
use katana_primitives::transaction::{
    DeclareTx, DeclareTxV1, DeclareTxV2, DeclareTxWithClass, DeployAccountTx, InvokeTx, TxHash,
    TxWithHash,
};
use katana_primitives::FieldElement;
use serde::{Deserialize, Serialize};
use starknet::core::types::{
    BroadcastedDeclareTransaction, BroadcastedDeployAccountTransaction,
    BroadcastedInvokeTransaction, DeclareTransactionResult, DeployAccountTransactionResult,
    InvokeTransactionResult,
};
use starknet::core::utils::get_contract_address;

#[derive(Debug, Clone, Serialize, Deserialize, Deref)]
#[serde(transparent)]
pub struct BroadcastedInvokeTx(BroadcastedInvokeTransaction);

impl BroadcastedInvokeTx {
    pub fn into_tx_with_chain_id(self, chain_id: ChainId) -> InvokeTx {
        InvokeTx {
            chain_id,
            nonce: self.0.nonce,
            calldata: self.0.calldata,
            signature: self.0.signature,
            version: FieldElement::ONE,
            sender_address: self.0.sender_address.into(),
            max_fee: self.0.max_fee.try_into().expect("max_fee is too big"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Deref)]
#[serde(transparent)]
pub struct BroadcastedDeclareTx(BroadcastedDeclareTransaction);

impl BroadcastedDeclareTx {
    /// Validates that the provided compiled class hash is computed correctly from the class
    /// provided in the transaction.
    pub fn validate_compiled_class_hash(&self) -> Result<bool> {
        let res = match &self.0 {
            BroadcastedDeclareTransaction::V1(_) => true,
            BroadcastedDeclareTransaction::V2(tx) => {
                let hash = compiled_class_hash_from_flattened_sierra_class(&tx.contract_class)?;
                hash == tx.compiled_class_hash
            }
        };
        Ok(res)
    }

    /// This function assumes that the compiled class hash is valid.
    pub fn try_into_tx_with_chain_id(self, chain_id: ChainId) -> Result<DeclareTxWithClass> {
        match self.0 {
            BroadcastedDeclareTransaction::V1(tx) => {
                let (class_hash, compiled_class) =
                    legacy_rpc_to_compiled_class(&tx.contract_class)?;

                Ok(DeclareTxWithClass {
                    compiled_class,
                    sierra_class: None,
                    transaction: DeclareTx::V1(DeclareTxV1 {
                        chain_id,
                        class_hash,
                        nonce: tx.nonce,
                        signature: tx.signature,
                        sender_address: tx.sender_address.into(),
                        max_fee: tx.max_fee.try_into().expect("max fee is too large"),
                    }),
                })
            }

            BroadcastedDeclareTransaction::V2(tx) => {
                // TODO: avoid computing the class hash again
                let (class_hash, _, compiled_class) =
                    flattened_sierra_to_compiled_class(&tx.contract_class)?;

                Ok(DeclareTxWithClass {
                    compiled_class,
                    sierra_class: Arc::into_inner(tx.contract_class),
                    transaction: DeclareTx::V2(DeclareTxV2 {
                        chain_id,
                        class_hash,
                        nonce: tx.nonce,
                        signature: tx.signature,
                        sender_address: tx.sender_address.into(),
                        compiled_class_hash: tx.compiled_class_hash,
                        max_fee: tx.max_fee.try_into().expect("max fee is too large"),
                    }),
                })
            }
        }
    }

    pub fn is_query(&self) -> bool {
        match &self.0 {
            BroadcastedDeclareTransaction::V1(tx) => tx.is_query,
            BroadcastedDeclareTransaction::V2(tx) => tx.is_query,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Deref)]
#[serde(transparent)]
pub struct BroadcastedDeployAccountTx(BroadcastedDeployAccountTransaction);

impl BroadcastedDeployAccountTx {
    pub fn into_tx_with_chain_id(self, chain_id: ChainId) -> DeployAccountTx {
        let contract_address = get_contract_address(
            self.0.contract_address_salt,
            self.0.class_hash,
            &self.0.constructor_calldata,
            FieldElement::ZERO,
        );

        DeployAccountTx {
            chain_id,
            nonce: self.0.nonce,
            version: FieldElement::ONE,
            signature: self.0.signature,
            class_hash: self.0.class_hash,
            contract_address: contract_address.into(),
            constructor_calldata: self.0.constructor_calldata,
            contract_address_salt: self.0.contract_address_salt,
            max_fee: self.0.max_fee.try_into().expect("max_fee is too big"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BroadcastedTx {
    Invoke(BroadcastedInvokeTx),
    Declare(BroadcastedDeclareTx),
    DeployAccount(BroadcastedDeployAccountTx),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tx(pub starknet::core::types::Transaction);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeployAccountTxResult(DeployAccountTransactionResult);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeclareTxResult(DeclareTransactionResult);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvokeTxResult(InvokeTransactionResult);

impl From<TxWithHash> for Tx {
    fn from(value: TxWithHash) -> Self {
        use katana_primitives::transaction::Tx as InternalTx;

        let transaction_hash = value.hash;
        let tx = match value.transaction {
            InternalTx::Invoke(tx) => starknet::core::types::Transaction::Invoke(
                starknet::core::types::InvokeTransaction::V1(
                    starknet::core::types::InvokeTransactionV1 {
                        nonce: tx.nonce,
                        transaction_hash,
                        calldata: tx.calldata,
                        signature: tx.signature,
                        max_fee: tx.max_fee.into(),
                        sender_address: tx.sender_address.into(),
                    },
                ),
            ),

            InternalTx::Declare(tx) => starknet::core::types::Transaction::Declare(match tx {
                DeclareTx::V1(tx) => starknet::core::types::DeclareTransaction::V1(
                    starknet::core::types::DeclareTransactionV1 {
                        nonce: tx.nonce,
                        transaction_hash,
                        signature: tx.signature,
                        class_hash: tx.class_hash,
                        max_fee: tx.max_fee.into(),
                        sender_address: tx.sender_address.into(),
                    },
                ),

                DeclareTx::V2(tx) => starknet::core::types::DeclareTransaction::V2(
                    starknet::core::types::DeclareTransactionV2 {
                        nonce: tx.nonce,
                        transaction_hash,
                        signature: tx.signature,
                        class_hash: tx.class_hash,
                        max_fee: tx.max_fee.into(),
                        sender_address: tx.sender_address.into(),
                        compiled_class_hash: tx.compiled_class_hash,
                    },
                ),
            }),

            InternalTx::L1Handler(tx) => starknet::core::types::Transaction::L1Handler(
                starknet::core::types::L1HandlerTransaction {
                    transaction_hash,
                    calldata: tx.calldata,
                    contract_address: tx.contract_address.into(),
                    entry_point_selector: tx.entry_point_selector,
                    nonce: tx.nonce.try_into().expect("nonce should fit in u64"),
                    version: tx.version.try_into().expect("version should fit in u64"),
                },
            ),

            InternalTx::DeployAccount(tx) => starknet::core::types::Transaction::DeployAccount(
                starknet::core::types::DeployAccountTransaction {
                    transaction_hash,
                    nonce: tx.nonce,
                    signature: tx.signature,
                    class_hash: tx.class_hash,
                    max_fee: tx.max_fee.into(),
                    constructor_calldata: tx.constructor_calldata,
                    contract_address_salt: tx.contract_address_salt,
                },
            ),
        };

        Tx(tx)
    }
}

impl DeployAccountTxResult {
    pub fn new(transaction_hash: TxHash, contract_address: ContractAddress) -> Self {
        Self(DeployAccountTransactionResult {
            transaction_hash,
            contract_address: contract_address.into(),
        })
    }
}

impl DeclareTxResult {
    pub fn new(transaction_hash: TxHash, class_hash: ClassHash) -> Self {
        Self(DeclareTransactionResult { transaction_hash, class_hash })
    }
}

impl InvokeTxResult {
    pub fn new(transaction_hash: TxHash) -> Self {
        Self(InvokeTransactionResult { transaction_hash })
    }
}

impl From<(TxHash, ContractAddress)> for DeployAccountTxResult {
    fn from((transaction_hash, contract_address): (TxHash, ContractAddress)) -> Self {
        Self::new(transaction_hash, contract_address)
    }
}

impl From<(TxHash, ClassHash)> for DeclareTxResult {
    fn from((transaction_hash, class_hash): (TxHash, ClassHash)) -> Self {
        Self::new(transaction_hash, class_hash)
    }
}

impl From<TxHash> for InvokeTxResult {
    fn from(transaction_hash: TxHash) -> Self {
        Self::new(transaction_hash)
    }
}

impl From<BroadcastedInvokeTx> for InvokeTx {
    fn from(tx: BroadcastedInvokeTx) -> Self {
        InvokeTx {
            nonce: tx.0.nonce,
            calldata: tx.0.calldata,
            signature: tx.0.signature,
            version: FieldElement::ONE,
            chain_id: ChainId::default(),
            sender_address: tx.0.sender_address.into(),
            max_fee: tx.0.max_fee.try_into().expect("max_fee is too big"),
        }
    }
}

impl From<BroadcastedDeployAccountTx> for DeployAccountTx {
    fn from(tx: BroadcastedDeployAccountTx) -> Self {
        let contract_address = get_contract_address(
            tx.0.contract_address_salt,
            tx.0.class_hash,
            &tx.0.constructor_calldata,
            FieldElement::ZERO,
        );

        DeployAccountTx {
            nonce: tx.0.nonce,
            signature: tx.0.signature,
            version: FieldElement::ONE,
            class_hash: tx.0.class_hash,
            chain_id: ChainId::default(),
            contract_address: contract_address.into(),
            constructor_calldata: tx.0.constructor_calldata,
            contract_address_salt: tx.0.contract_address_salt,
            max_fee: tx.0.max_fee.try_into().expect("max_fee is too big"),
        }
    }
}
