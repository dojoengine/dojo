use serde::{Deserialize, Serialize};
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, NonceUpdate, StorageEntry,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybePendingStateUpdate {
    Pending(PendingStateUpdate),
    Update(StateUpdate),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StateUpdate(starknet::core::types::StateUpdate);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PendingStateUpdate(starknet::core::types::PendingStateUpdate);

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct StateDiff(pub starknet::core::types::StateDiff);

impl From<starknet::core::types::StateUpdate> for StateUpdate {
    fn from(value: starknet::core::types::StateUpdate) -> Self {
        StateUpdate(value)
    }
}

impl From<katana_primitives::state::StateUpdates> for StateDiff {
    fn from(value: katana_primitives::state::StateUpdates) -> Self {
        let nonces: Vec<NonceUpdate> = value
            .nonce_updates
            .into_iter()
            .map(|(addr, nonce)| NonceUpdate { nonce, contract_address: addr.into() })
            .collect();

        let declared_classes: Vec<DeclaredClassItem> = value
            .declared_classes
            .into_iter()
            .map(|(class_hash, compiled_class_hash)| DeclaredClassItem {
                class_hash,
                compiled_class_hash,
            })
            .collect();

        let deployed_contracts: Vec<DeployedContractItem> = value
            .contract_updates
            .into_iter()
            .map(|(addr, class_hash)| DeployedContractItem { address: addr.into(), class_hash })
            .collect();

        let storage_diffs: Vec<ContractStorageDiffItem> = value
            .storage_updates
            .into_iter()
            .map(|(addr, entries)| ContractStorageDiffItem {
                address: addr.into(),
                storage_entries: entries
                    .into_iter()
                    .map(|(key, value)| StorageEntry { key, value })
                    .collect(),
            })
            .collect();

        StateDiff(starknet::core::types::StateDiff {
            nonces,
            storage_diffs,
            declared_classes,
            deployed_contracts,
            replaced_classes: Default::default(),
            deprecated_declared_classes: Default::default(),
        })
    }
}
