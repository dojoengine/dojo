use std::{fs, path::PathBuf};

use anyhow::Result;
use blockifier::{
    execution::contract_class::ContractClass,
    transaction::{
        account_transaction::AccountTransaction,
        transaction_execution::Transaction as BlockifierTransaction,
    },
};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet_api::{
    core::ClassHash,
    hash::StarkFelt,
    transaction::{
        DeclareTransaction, DeclareTransactionV0V1, DeclareTransactionV2, DeployAccountTransaction,
        InvokeTransaction, InvokeTransactionV1, L1HandlerTransaction, Transaction,
    },
};

pub fn get_contract_class(contract_path: &str) -> ContractClass {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), contract_path].iter().collect();
    let raw_contract_class = fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw_contract_class).unwrap()
}

pub fn convert_blockifier_tx_to_starknet_api_tx(
    transaction: &BlockifierTransaction,
) -> Transaction {
    match transaction {
        BlockifierTransaction::AccountTransaction(tx) => match tx {
            AccountTransaction::Invoke(tx) => {
                Transaction::Invoke(InvokeTransaction::V1(InvokeTransactionV1 {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee,
                    calldata: tx.calldata.clone(),
                    signature: tx.signature.clone(),
                    sender_address: tx.sender_address,
                    transaction_hash: tx.transaction_hash,
                }))
            }
            AccountTransaction::DeployAccount(tx) => {
                Transaction::DeployAccount(DeployAccountTransaction {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee,
                    version: tx.version,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    transaction_hash: tx.transaction_hash,
                    contract_address: tx.contract_address,
                    contract_address_salt: tx.contract_address_salt,
                    constructor_calldata: tx.constructor_calldata.clone(),
                })
            }
            AccountTransaction::Declare(tx, _) => match tx {
                DeclareTransaction::V0(tx) => {
                    Transaction::Declare(DeclareTransaction::V0(DeclareTransactionV0V1 {
                        nonce: tx.nonce,
                        max_fee: tx.max_fee,
                        class_hash: tx.class_hash,
                        signature: tx.signature.clone(),
                        sender_address: tx.sender_address,
                        transaction_hash: tx.transaction_hash,
                    }))
                }

                DeclareTransaction::V1(tx) => {
                    Transaction::Declare(DeclareTransaction::V1(DeclareTransactionV0V1 {
                        nonce: tx.nonce,
                        max_fee: tx.max_fee,
                        class_hash: tx.class_hash,
                        signature: tx.signature.clone(),
                        sender_address: tx.sender_address,
                        transaction_hash: tx.transaction_hash,
                    }))
                }

                DeclareTransaction::V2(tx) => {
                    Transaction::Declare(DeclareTransaction::V2(DeclareTransactionV2 {
                        nonce: tx.nonce,
                        max_fee: tx.max_fee,
                        class_hash: tx.class_hash,
                        signature: tx.signature.clone(),
                        sender_address: tx.sender_address,
                        transaction_hash: tx.transaction_hash,
                        compiled_class_hash: tx.compiled_class_hash,
                    }))
                }
            },
        },
        BlockifierTransaction::L1HandlerTransaction(tx) => {
            Transaction::L1Handler(L1HandlerTransaction {
                nonce: tx.nonce,
                version: tx.version,
                calldata: tx.calldata.clone(),
                transaction_hash: tx.transaction_hash,
                contract_address: tx.contract_address,
                entry_point_selector: tx.entry_point_selector,
            })
        }
    }
}

pub fn compute_legacy_class_hash(contract_class_str: &str) -> Result<ClassHash> {
    let contract_class: LegacyContractClass = ::serde_json::from_str(contract_class_str)?;
    let seirra_class_hash = contract_class.class_hash()?;
    Ok(ClassHash(StarkFelt::from(seirra_class_hash)))
}
