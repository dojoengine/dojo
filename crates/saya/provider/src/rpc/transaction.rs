//! Transactions related conversions.
use katana_primitives::chain::ChainId;
use katana_primitives::transaction::{
    DeclareTx, DeclareTxV1, DeclareTxV2, DeployAccountTx, InvokeTx, L1HandlerTx, Tx, TxWithHash,
};
use starknet::core::types::{DeclareTransaction, FieldElement, InvokeTransaction, Transaction};

use crate::ProviderResult;

pub fn tx_from_rpc(tx_rpc: &Transaction, chain_id: ChainId) -> ProviderResult<TxWithHash> {
    match tx_rpc {
        Transaction::Invoke(tx_e) => match tx_e {
            InvokeTransaction::V0(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: {
                    Tx::Invoke(InvokeTx {
                        max_fee: tx.max_fee.try_into()?,
                        chain_id,
                        version: FieldElement::ZERO,
                        calldata: tx.calldata.clone(),
                        signature: tx.signature.clone(),
                        ..Default::default()
                    })
                },
            }),
            InvokeTransaction::V1(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Invoke(InvokeTx {
                    max_fee: tx.max_fee.try_into()?,
                    chain_id,
                    version: FieldElement::ZERO,
                    calldata: tx.calldata.clone(),
                    signature: tx.signature.clone(),
                    nonce: tx.nonce,
                    sender_address: tx.sender_address.into(),
                }),
            }),
        },
        Transaction::L1Handler(tx) => {
            // Seems we have data loss from only this content from the transaction.
            // The receipt may be required to complete the data.
            // (or use directly the database...)
            Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::L1Handler(L1HandlerTx {
                    nonce: tx.nonce.into(),
                    chain_id,
                    version: FieldElement::ZERO,
                    calldata: tx.calldata.clone(),
                    contract_address: tx.contract_address.into(),
                    entry_point_selector: tx.entry_point_selector,
                    ..Default::default()
                }),
            })
        }
        Transaction::Declare(tx_e) => match tx_e {
            DeclareTransaction::V0(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Declare(DeclareTx::V1(DeclareTxV1 {
                    max_fee: tx.max_fee.try_into()?,
                    chain_id,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    sender_address: tx.sender_address.into(),
                    ..Default::default()
                })),
            }),
            DeclareTransaction::V1(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Declare(DeclareTx::V1(DeclareTxV1 {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee.try_into()?,
                    chain_id,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    sender_address: tx.sender_address.into(),
                })),
            }),
            DeclareTransaction::V2(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Declare(DeclareTx::V2(DeclareTxV2 {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee.try_into()?,
                    chain_id,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    sender_address: tx.sender_address.into(),
                    compiled_class_hash: tx.compiled_class_hash,
                })),
            }),
        },
        Transaction::DeployAccount(tx) => {
            // contract_address field is missing, to be checked.
            Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::DeployAccount(DeployAccountTx {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee.try_into()?,
                    chain_id,
                    version: FieldElement::ONE,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    contract_address_salt: tx.contract_address_salt,
                    constructor_calldata: tx.constructor_calldata.clone(),
                    ..Default::default()
                }),
            })
        }
        Transaction::Deploy(_) => {
            panic!("Deploy transaction not supported");
        }
    }
}
