pub mod contract;
pub mod event;
pub mod transaction;

use std::time::{Duration, SystemTime};

use anyhow::Result;
use blockifier::state::cached_state::CommitmentStateDiff;
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, NonceUpdate, StateDiff,
    StorageEntry,
};
use starknet_api::hash::StarkFelt;

use starknet_api::StarknetApiError;

pub fn get_current_timestamp() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("should get current UNIX timestamp")
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
