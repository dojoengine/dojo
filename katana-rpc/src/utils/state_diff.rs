use anyhow::Result;
use starknet::providers::jsonrpc::models::{
    ContractStorageDiffItem, DeployedContractItem, NonceUpdate, StateDiff, StorageEntry,
};

use super::transaction::stark_felt_to_field_element;

pub fn convert_state_diff_to_rpc_state_diff(
    state_diff: starknet_api::state::StateDiff,
) -> Result<StateDiff> {
    Ok(StateDiff {
        storage_diffs: state_diff
            .storage_diffs
            .iter()
            .map(|(address, entries)| ContractStorageDiffItem {
                address: stark_felt_to_field_element(*address.0.key()).unwrap(),
                storage_entries: entries
                    .iter()
                    .map(|(key, value)| StorageEntry {
                        key: stark_felt_to_field_element(*key.0.key()).unwrap(),
                        value: stark_felt_to_field_element(*value).unwrap(),
                    })
                    .collect(),
            })
            .collect(),
        // TODO: This will change with RPC spec v3.0.0. Also, are we supposed to return the class hash or the compiled class hash?
        declared_contract_hashes: state_diff
            .declared_classes
            .iter()
            .map(|class_hash| stark_felt_to_field_element(class_hash.0 .0).unwrap())
            .collect(),
        deployed_contracts: state_diff
            .deployed_contracts
            .iter()
            .map(|(address, class_hash)| DeployedContractItem {
                address: stark_felt_to_field_element(*address.0.key()).unwrap(),
                class_hash: stark_felt_to_field_element(class_hash.0).unwrap(),
            })
            .collect(),
        nonces: state_diff
            .nonces
            .iter()
            .map(|(address, nonce)| NonceUpdate {
                contract_address: stark_felt_to_field_element(*address.0.key()).unwrap(),
                nonce: stark_felt_to_field_element(nonce.0).unwrap(),
            })
            .collect(),
    })
}
