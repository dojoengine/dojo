use std::sync::Arc;

use ::blockifier::transaction::transaction_execution::Transaction;
use ::blockifier::transaction::transactions::DeployAccountTransaction;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transactions::{DeclareTransaction, L1HandlerTransaction};
use katana_primitives::transaction::{DeclareTx, ExecutableTx, ExecutableTxWithHash};
use starknet_api::core::{ClassHash, CompiledClassHash, EntryPointSelector, Nonce};
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeclareTransactionV0V1, DeclareTransactionV2, Fee,
    InvokeTransaction, InvokeTransactionV1, TransactionHash, TransactionSignature,
    TransactionVersion,
};

/// A newtype wrapper for execution transaction used in `blockifier`.
pub struct BlockifierTx(pub(super) ::blockifier::transaction::transaction_execution::Transaction);

impl From<ExecutableTxWithHash> for BlockifierTx {
    fn from(value: ExecutableTxWithHash) -> Self {
        let hash = value.hash;

        let tx = match value.transaction {
            ExecutableTx::Invoke(tx) => {
                let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();
                let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                Transaction::AccountTransaction(AccountTransaction::Invoke(InvokeTransaction::V1(
                    InvokeTransactionV1 {
                        max_fee: Fee(tx.max_fee),
                        nonce: Nonce(tx.nonce.into()),
                        sender_address: tx.sender_address.into(),
                        signature: TransactionSignature(signature),
                        calldata: Calldata(Arc::new(calldata)),
                        transaction_hash: TransactionHash(hash.into()),
                    },
                )))
            }

            ExecutableTx::DeployAccount(tx) => {
                let calldata = tx.constructor_calldata.into_iter().map(|f| f.into()).collect();
                let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                Transaction::AccountTransaction(AccountTransaction::DeployAccount(
                    DeployAccountTransaction {
                        contract_address: tx.contract_address.into(),
                        tx: starknet_api::transaction::DeployAccountTransaction {
                            max_fee: Fee(tx.max_fee),
                            nonce: Nonce(tx.nonce.into()),
                            version: TransactionVersion(1u128.into()),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                            transaction_hash: TransactionHash(hash.into()),
                            constructor_calldata: Calldata(Arc::new(calldata)),
                            contract_address_salt: ContractAddressSalt(
                                tx.contract_address_salt.into(),
                            ),
                        },
                    },
                ))
            }

            ExecutableTx::Declare(tx) => {
                let contract_class = tx.compiled_class;

                let tx = match tx.transaction {
                    DeclareTx::V1(tx) => {
                        let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                        starknet_api::transaction::DeclareTransaction::V1(DeclareTransactionV0V1 {
                            max_fee: Fee(tx.max_fee),
                            nonce: Nonce(tx.nonce.into()),
                            sender_address: tx.sender_address.into(),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                            transaction_hash: TransactionHash(hash.into()),
                        })
                    }

                    DeclareTx::V2(tx) => {
                        let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                        starknet_api::transaction::DeclareTransaction::V2(DeclareTransactionV2 {
                            max_fee: Fee(tx.max_fee),
                            nonce: Nonce(tx.nonce.into()),
                            sender_address: tx.sender_address.into(),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                            transaction_hash: TransactionHash(hash.into()),
                            compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                        })
                    }
                };

                let tx = DeclareTransaction::new(tx, contract_class).expect("class mismatch");
                Transaction::AccountTransaction(AccountTransaction::Declare(tx))
            }

            ExecutableTx::L1Handler(tx) => {
                let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();
                Transaction::L1HandlerTransaction(L1HandlerTransaction {
                    paid_fee_on_l1: Fee(tx.paid_fee_on_l1),
                    tx: starknet_api::transaction::L1HandlerTransaction {
                        nonce: Nonce(tx.nonce.into()),
                        calldata: Calldata(Arc::new(calldata)),
                        version: TransactionVersion(1u128.into()),
                        contract_address: tx.contract_address.into(),
                        transaction_hash: TransactionHash(hash.into()),
                        entry_point_selector: EntryPointSelector(tx.entry_point_selector.into()),
                    },
                })
            }
        };

        Self(tx)
    }
}
