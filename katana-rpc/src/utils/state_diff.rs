use anyhow::Result;
use starknet::providers::jsonrpc::models::{
    ContractStorageDiffItem, DeployedContractItem, NonceUpdate, StateDiff, StorageEntry,
};

pub fn convert_state_diff_to_rpc_state_diff(
    state_diff: starknet_api::state::StateDiff,
) -> Result<StateDiff> {
    Ok(StateDiff {
        storage_diffs: state_diff
            .storage_diffs
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
        // TODO: This will change with RPC spec v3.0.0. Also, are we supposed to return the class hash or the compiled class hash?
        declared_contract_hashes: state_diff
            .declared_classes
            .iter()
            .map(|class_hash| class_hash.0 .0.into())
            .collect(),
        deployed_contracts: state_diff
            .deployed_contracts
            .iter()
            .map(|(address, class_hash)| DeployedContractItem {
                address: (*address.0.key()).into(),
                class_hash: class_hash.0.into(),
            })
            .collect(),
        nonces: state_diff
            .nonces
            .iter()
            .map(|(address, nonce)| NonceUpdate {
                contract_address: (*address.0.key()).into(),
                nonce: nonce.0.into(),
            })
            .collect(),
    })
}
