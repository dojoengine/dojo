//! State updates.
use std::collections::HashMap;

use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, NonceUpdate, StateUpdate,
};

use crate::ProviderResult;

pub fn state_updates_from_rpc(state_update: &StateUpdate) -> ProviderResult<StateUpdates> {
    let mut out = StateUpdates::default();

    let state_diff = &state_update.state_diff;

    for contract_diff in &state_diff.storage_diffs {
        let ContractStorageDiffItem { address, storage_entries: entries } = contract_diff;

        let address: ContractAddress = (*address).into();

        let contract_entry = out.storage_updates.entry(address).or_insert_with(HashMap::new);

        for e in entries {
            contract_entry.insert(e.key, e.value);
        }
    }

    for nonce_update in &state_diff.nonces {
        let NonceUpdate { contract_address, nonce: new_nonce } = *nonce_update;
        out.nonce_updates.insert(contract_address.into(), new_nonce);
    }

    for deployed in &state_diff.deployed_contracts {
        let DeployedContractItem { address, class_hash } = *deployed;
        out.contract_updates.insert(address.into(), class_hash);
    }

    for decl in &state_diff.declared_classes {
        let DeclaredClassItem { class_hash, compiled_class_hash } = decl;
        out.declared_classes.insert((*class_hash).into(), *compiled_class_hash);
    }

    Ok(out)
}
