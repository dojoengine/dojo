//! Translation layer for converting the primitive types to the execution engine types.

use std::sync::Arc;

use blockifier::transaction::account_transaction::AccountTransaction;
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

use crate::transaction::ExecutionTx;
use crate::FieldElement;

mod primitives {
    pub use crate::contract::{ContractAddress, Nonce};
    pub use crate::transaction::{
        DeclareTx, DeclareTxWithClasses, DeployAccountTx, InvokeTx, L1HandlerTx,
    };
    pub use crate::FieldElement;
}

impl From<primitives::ContractAddress> for ContractAddress {
    fn from(address: primitives::ContractAddress) -> Self {
        Self(patricia_key!(address.0))
    }
}

impl From<ContractAddress> for primitives::ContractAddress {
    fn from(address: ContractAddress) -> Self {
        Self((*address.0.key()).into())
    }
}

impl From<primitives::InvokeTx> for InvokeTransaction {
    fn from(tx: primitives::InvokeTx) -> Self {
        if FieldElement::ONE == tx.version {
            InvokeTransaction::V1(InvokeTransactionV1 {
                max_fee: Fee(tx.max_fee),
                nonce: Nonce(tx.nonce.into()),
                sender_address: tx.sender_address.into(),
                calldata: calldata_from_felts_vec(tx.calldata),
                signature: signature_from_felts_vec(tx.signature),
                transaction_hash: TransactionHash(tx.transaction_hash.into()),
            })
        } else {
            unimplemented!("Unsupported transaction version")
        }
    }
}

impl From<primitives::DeclareTx> for DeclareTransaction {
    fn from(tx: primitives::DeclareTx) -> Self {
        if FieldElement::ONE == tx.version {
            DeclareTransaction::V1(DeclareTransactionV0V1 {
                max_fee: Fee(tx.max_fee),
                nonce: Nonce(tx.nonce.into()),
                sender_address: tx.sender_address.into(),
                class_hash: ClassHash(tx.class_hash.into()),
                signature: signature_from_felts_vec(tx.signature),
                transaction_hash: TransactionHash(tx.transaction_hash.into()),
            })
        } else if FieldElement::TWO == tx.version {
            DeclareTransaction::V2(DeclareTransactionV2 {
                max_fee: Fee(tx.max_fee),
                nonce: Nonce(tx.nonce.into()),
                sender_address: tx.sender_address.into(),
                class_hash: ClassHash(tx.class_hash.into()),
                signature: signature_from_felts_vec(tx.signature),
                transaction_hash: TransactionHash(tx.transaction_hash.into()),
                compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.unwrap().into()),
            })
        } else {
            unimplemented!("Unsupported transaction version")
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

impl From<primitives::DeployAccountTx>
    for blockifier::transaction::transactions::DeployAccountTransaction
{
    fn from(tx: primitives::DeployAccountTx) -> Self {
        Self { contract_address: tx.contract_address.into(), tx: tx.into() }
    }
}

impl From<primitives::DeclareTxWithClasses>
    for blockifier::transaction::transactions::DeclareTransaction
{
    fn from(tx: primitives::DeclareTxWithClasses) -> Self {
        Self::new(tx.tx.into(), tx.compiled_class).expect("tx & class must be compatible")
    }
}

impl From<ExecutionTx> for blockifier::transaction::transaction_execution::Transaction {
    fn from(tx: ExecutionTx) -> Self {
        match tx {
            ExecutionTx::L1Handler(tx) => Self::L1HandlerTransaction(
                blockifier::transaction::transactions::L1HandlerTransaction {
                    paid_fee_on_l1: Fee(tx.paid_fee_on_l1),
                    tx: tx.into(),
                },
            ),

            ExecutionTx::Invoke(tx) => {
                Self::AccountTransaction(AccountTransaction::Invoke(tx.into()))
            }
            ExecutionTx::Declare(tx) => {
                Self::AccountTransaction(AccountTransaction::Declare(tx.into()))
            }
            ExecutionTx::DeployAccount(tx) => {
                Self::AccountTransaction(AccountTransaction::DeployAccount(tx.into()))
            }
        }
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
