//! Translation layer for converting the primitive types to the execution engine types.

use std::sync::Arc;

use starknet_api::core::{
    ClassHash, CompiledClassHash, ContractAddress, EntryPointSelector, Nonce, PatriciaKey,
};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::patricia_key;
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeclareTransaction, DeclareTransactionV0V1,
    DeclareTransactionV2, DeployAccountTransaction, Fee, InvokeTransaction, InvokeTransactionV1,
    L1HandlerTransaction, TransactionHash, TransactionSignature, TransactionVersion,
};

mod primitives {
    pub use crate::contract::{ContractAddress, Nonce};
    pub use crate::transaction::{
        DeclareTx, DeclareTxV1, DeclareTxV2, DeclareTxWithCompiledClass, DeployAccountTx,
        DeployAccountTxWithContractAddress, InvokeTx, InvokeTxV1, L1HandlerTx,
    };
    pub use crate::FieldElement;
}

impl From<primitives::ContractAddress> for ContractAddress {
    fn from(address: primitives::ContractAddress) -> Self {
        Self(patricia_key!(address.0))
    }
}

impl From<primitives::InvokeTx> for InvokeTransaction {
    fn from(tx: primitives::InvokeTx) -> Self {
        match tx {
            primitives::InvokeTx::V1(tx) => InvokeTransaction::V1(tx.into()),
        }
    }
}

impl From<primitives::InvokeTxV1> for InvokeTransactionV1 {
    fn from(tx: primitives::InvokeTxV1) -> Self {
        Self {
            max_fee: Fee(tx.max_fee),
            nonce: Nonce(tx.nonce.into()),
            sender_address: tx.sender_address.into(),
            calldata: calldata_from_felts_vec(tx.calldata),
            signature: signature_from_felts_vec(tx.signature),
            transaction_hash: TransactionHash(tx.transaction_hash.into()),
        }
    }
}

impl From<primitives::DeclareTx> for DeclareTransaction {
    fn from(tx: primitives::DeclareTx) -> Self {
        match tx {
            primitives::DeclareTx::V1(tx) => DeclareTransaction::V1(tx.into()),
            primitives::DeclareTx::V2(tx) => DeclareTransaction::V2(tx.into()),
        }
    }
}

impl From<primitives::DeclareTxV1> for DeclareTransactionV0V1 {
    fn from(tx: primitives::DeclareTxV1) -> Self {
        Self {
            max_fee: Fee(tx.max_fee),
            nonce: Nonce(tx.nonce.into()),
            sender_address: tx.sender_address.into(),
            class_hash: ClassHash(tx.class_hash.into()),
            signature: signature_from_felts_vec(tx.signature),
            transaction_hash: TransactionHash(tx.transaction_hash.into()),
        }
    }
}

impl From<primitives::DeclareTxV2> for DeclareTransactionV2 {
    fn from(tx: primitives::DeclareTxV2) -> Self {
        Self {
            max_fee: Fee(tx.max_fee),
            nonce: Nonce(tx.nonce.into()),
            sender_address: tx.sender_address.into(),
            class_hash: ClassHash(tx.class_hash.into()),
            signature: signature_from_felts_vec(tx.signature),
            transaction_hash: TransactionHash(tx.transaction_hash.into()),
            compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
        }
    }
}

impl From<primitives::DeployAccountTx> for DeployAccountTransaction {
    fn from(tx: primitives::DeployAccountTx) -> Self {
        Self {
            max_fee: Fee(tx.max_fee),
            nonce: Nonce(tx.nonce.into()),
            class_hash: ClassHash(tx.class_hash.into()),
            version: TransactionVersion(tx.version.into()),
            signature: signature_from_felts_vec(tx.signature),
            transaction_hash: TransactionHash(tx.transaction_hash.into()),
            contract_address_salt: ContractAddressSalt(tx.contract_address_salt.into()),
            constructor_calldata: calldata_from_felts_vec(tx.constructor_calldata),
        }
    }
}

impl From<primitives::L1HandlerTx> for L1HandlerTransaction {
    fn from(tx: primitives::L1HandlerTx) -> Self {
        Self {
            nonce: Nonce(tx.nonce.into()),
            contract_address: tx.contract_address.into(),
            version: TransactionVersion(tx.version.into()),
            calldata: calldata_from_felts_vec(tx.calldata),
            transaction_hash: TransactionHash(tx.transaction_hash.into()),
            entry_point_selector: EntryPointSelector(tx.entry_point_selector.into()),
        }
    }
}

impl From<primitives::DeployAccountTxWithContractAddress>
    for blockifier::transaction::transactions::DeployAccountTransaction
{
    fn from(tx: primitives::DeployAccountTxWithContractAddress) -> Self {
        Self { tx: tx.0.into(), contract_address: tx.1.into() }
    }
}

impl From<primitives::DeclareTxWithCompiledClass>
    for blockifier::transaction::transactions::DeclareTransaction
{
    fn from(tx: primitives::DeclareTxWithCompiledClass) -> Self {
        Self::new(tx.0.into(), tx.1).expect("tx & class must be compatible")
    }
}

#[inline]
fn felt_to_starkfelt_vec(felts: Vec<primitives::FieldElement>) -> Vec<StarkFelt> {
    felts.into_iter().map(|f| f.into()).collect()
}

#[inline]
fn calldata_from_felts_vec(felts: Vec<primitives::FieldElement>) -> Calldata {
    Calldata(Arc::new(felt_to_starkfelt_vec(felts)))
}

#[inline]
fn signature_from_felts_vec(felts: Vec<primitives::FieldElement>) -> TransactionSignature {
    TransactionSignature(felt_to_starkfelt_vec(felts))
}
