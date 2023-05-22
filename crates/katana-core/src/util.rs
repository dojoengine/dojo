use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use blockifier::execution::contract_class::{
    ContractClass, ContractClassV0, ContractClassV1 as BlockifierContractClass,
};
use blockifier::state::cached_state::CommitmentStateDiff;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction as BlockifierTransaction;
use blockifier::transaction::transactions::DeclareTransaction;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, FieldElement, NonceUpdate,
    StateDiff, StorageEntry,
};
use starknet_api::core::ClassHash;
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::{
    DeployAccountTransaction, InvokeTransaction, InvokeTransactionV1, L1HandlerTransaction,
    Transaction,
};
use starknet_api::StarknetApiError;

pub fn get_current_timestamp() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("should get current UNIX timestamp")
}

pub fn get_contract_class(contract_path: &str) -> ContractClass {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), contract_path].iter().collect();
    let raw_contract_class = fs::read_to_string(path).unwrap();
    let legacy_contract_class: ContractClassV0 = serde_json::from_str(&raw_contract_class).unwrap();
    ContractClass::V0(legacy_contract_class)
}

pub fn convert_blockifier_tx_to_starknet_api_tx(
    transaction: &BlockifierTransaction,
) -> Transaction {
    match transaction {
        BlockifierTransaction::AccountTransaction(tx) => match tx {
            AccountTransaction::Invoke(tx) => {
                Transaction::Invoke(InvokeTransaction::V1(InvokeTransactionV1 {
                    nonce: tx.nonce(),
                    max_fee: tx.max_fee(),
                    calldata: tx.calldata(),
                    signature: tx.signature(),
                    sender_address: tx.sender_address(),
                    transaction_hash: tx.transaction_hash(),
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
            AccountTransaction::Declare(DeclareTransaction { tx, .. }) => match tx {
                starknet_api::transaction::DeclareTransaction::V0(tx) => {
                    Transaction::Declare(starknet_api::transaction::DeclareTransaction::V0(
                        starknet_api::transaction::DeclareTransactionV0V1 {
                            nonce: tx.nonce,
                            max_fee: tx.max_fee,
                            class_hash: tx.class_hash,
                            signature: tx.signature.clone(),
                            sender_address: tx.sender_address,
                            transaction_hash: tx.transaction_hash,
                        },
                    ))
                }

                starknet_api::transaction::DeclareTransaction::V1(tx) => {
                    Transaction::Declare(starknet_api::transaction::DeclareTransaction::V1(
                        starknet_api::transaction::DeclareTransactionV0V1 {
                            nonce: tx.nonce,
                            max_fee: tx.max_fee,
                            class_hash: tx.class_hash,
                            signature: tx.signature.clone(),
                            sender_address: tx.sender_address,
                            transaction_hash: tx.transaction_hash,
                        },
                    ))
                }

                starknet_api::transaction::DeclareTransaction::V2(tx) => {
                    Transaction::Declare(starknet_api::transaction::DeclareTransaction::V2(
                        starknet_api::transaction::DeclareTransactionV2 {
                            nonce: tx.nonce,
                            max_fee: tx.max_fee,
                            class_hash: tx.class_hash,
                            signature: tx.signature.clone(),
                            sender_address: tx.sender_address,
                            transaction_hash: tx.transaction_hash,
                            compiled_class_hash: tx.compiled_class_hash,
                        },
                    ))
                }
            },
        },
        BlockifierTransaction::L1HandlerTransaction(l1_tx) => {
            Transaction::L1Handler(L1HandlerTransaction {
                nonce: l1_tx.tx.nonce,
                version: l1_tx.tx.version,
                calldata: l1_tx.tx.calldata.clone(),
                transaction_hash: l1_tx.tx.transaction_hash,
                contract_address: l1_tx.tx.contract_address,
                entry_point_selector: l1_tx.tx.entry_point_selector,
            })
        }
    }
}

pub fn compute_legacy_class_hash(contract_class_str: &str) -> Result<ClassHash> {
    let contract_class: LegacyContractClass = ::serde_json::from_str(contract_class_str)?;
    let seirra_class_hash = contract_class.class_hash()?;
    Ok(ClassHash(field_element_to_starkfelt(&seirra_class_hash)))
}

pub fn field_element_to_starkfelt(field_element: &FieldElement) -> StarkFelt {
    StarkFelt::new(field_element.to_bytes_be())
        .expect("must be able to convert to StarkFelt from FieldElement")
}

pub fn starkfelt_to_u128(felt: StarkFelt) -> Result<u128, StarknetApiError> {
    const COMPLIMENT_OF_U128: usize =
        std::mem::size_of::<StarkFelt>() - std::mem::size_of::<u128>();

    let (rest, u128_bytes) = felt.bytes().split_at(COMPLIMENT_OF_U128);
    if rest != [0u8; COMPLIMENT_OF_U128] {
        Err(StarknetApiError::OutOfRange { string: felt.to_string() })
    } else {
        Ok(u128::from_be_bytes(u128_bytes.try_into().expect("u128_bytes should be of size usize.")))
    }
}

pub fn blockifier_contract_class_from_flattened_sierra_class(
    raw_contract_class: &str,
) -> Result<BlockifierContractClass> {
    let value = serde_json::from_str::<serde_json::Value>(raw_contract_class)?;
    let contract_class = cairo_lang_starknet::contract_class::ContractClass {
        abi: serde_json::from_value(value["abi"].clone()).ok(),
        sierra_program: serde_json::from_value(value["sierra_program"].clone())?,
        entry_points_by_type: serde_json::from_value(value["entry_points_by_type"].clone())?,
        contract_class_version: serde_json::from_value(value["contract_class_version"].clone())?,
        sierra_program_debug_info: serde_json::from_value(
            value["sierra_program_debug_info"].clone(),
        )
        .ok(),
    };

    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;
    Ok(casm_contract.try_into()?)
}

pub fn convert_state_diff_to_rpc_state_diff(state_diff: CommitmentStateDiff) -> StateDiff {
    StateDiff {
        storage_diffs: state_diff
            .storage_updates
            .iter()
            .map(|(address, entries)| ContractStorageDiffItem {
                address: (*address.0.key()).into(),
                storage_entries: entries
                    .iter()
                    .map(|(key, value)| StorageEntry {
                        key: (*key.0.key()).into(),
                        value: (*value).into(),
                    })
                    .collect(),
            })
            .collect(),
        deprecated_declared_classes: vec![],
        // TODO: This will change with RPC spec v3.0.0. Also, are we supposed to return the class
        // hash or the compiled class hash?
        declared_classes: state_diff
            .class_hash_to_compiled_class_hash
            .iter()
            .map(|(class_hash, compiled_class_hash)| DeclaredClassItem {
                class_hash: class_hash.0.into(),
                compiled_class_hash: compiled_class_hash.0.into(),
            })
            .collect(),
        deployed_contracts: state_diff
            .address_to_class_hash
            .iter()
            .map(|(address, class_hash)| DeployedContractItem {
                address: (*address.0.key()).into(),
                class_hash: class_hash.0.into(),
            })
            .collect(),
        replaced_classes: vec![],
        nonces: state_diff
            .address_to_nonce
            .iter()
            .map(|(address, nonce)| NonceUpdate {
                contract_address: (*address.0.key()).into(),
                nonce: nonce.0.into(),
            })
            .collect(),
    }
}
