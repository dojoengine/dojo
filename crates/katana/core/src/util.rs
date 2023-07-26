use core::fmt;
use std::num::ParseIntError;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use blockifier::state::cached_state::CommitmentStateDiff;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction as BlockifierTransaction;
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
use thiserror::Error;

pub fn get_current_timestamp() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("should get current UNIX timestamp")
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
            AccountTransaction::Declare(tx) => match tx.tx() {
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

#[derive(PartialEq, Eq, Debug, Default)]
pub struct ContinuationToken {
    pub block_n: u64,
    pub txn_n: u64,
    pub event_n: u64,
}

#[derive(PartialEq, Eq, Debug, Error)]
pub enum ContinuationTokenError {
    #[error("Invalid data")]
    InvalidToken,
    #[error("Invalid format: {0}")]
    ParseFailed(ParseIntError),
}

impl ContinuationToken {
    pub fn parse(token: String) -> Result<Self, ContinuationTokenError> {
        let arr: Vec<&str> = token.split(',').collect();
        if arr.len() != 3 {
            return Err(ContinuationTokenError::InvalidToken);
        }
        let block_n =
            u64::from_str_radix(arr[0], 16).map_err(ContinuationTokenError::ParseFailed)?;
        let receipt_n =
            u64::from_str_radix(arr[1], 16).map_err(ContinuationTokenError::ParseFailed)?;
        let event_n =
            u64::from_str_radix(arr[2], 16).map_err(ContinuationTokenError::ParseFailed)?;

        Ok(ContinuationToken { block_n, txn_n: receipt_n, event_n })
    }
}

impl fmt::Display for ContinuationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x},{:x},{:x}", self.block_n, self.txn_n, self.event_n)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn to_string_works() {
        fn helper(block_n: u64, txn_n: u64, event_n: u64) -> String {
            ContinuationToken { block_n, txn_n, event_n }.to_string()
        }

        assert_eq!(helper(0, 0, 0), "0,0,0");
        assert_eq!(helper(30, 255, 4), "1e,ff,4");
    }

    #[test]
    fn parse_works() {
        fn helper(token: &str) -> ContinuationToken {
            ContinuationToken::parse(token.to_owned()).unwrap()
        }
        assert_eq!(helper("0,0,0"), ContinuationToken { block_n: 0, txn_n: 0, event_n: 0 });
        assert_eq!(helper("1e,ff,4"), ContinuationToken { block_n: 30, txn_n: 255, event_n: 4 });
    }

    #[test]
    fn parse_should_fail() {
        assert_eq!(
            ContinuationToken::parse("100".to_owned()).unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
        assert_eq!(
            ContinuationToken::parse("0,".to_owned()).unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
        assert_eq!(
            ContinuationToken::parse("0,0".to_owned()).unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
    }

    #[test]
    fn parse_u64_should_fail() {
        matches!(
            ContinuationToken::parse("2y,100,4".to_owned()).unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
        matches!(
            ContinuationToken::parse("30,255g,4".to_owned()).unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
        matches!(
            ContinuationToken::parse("244,1,fv".to_owned()).unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
    }
}
