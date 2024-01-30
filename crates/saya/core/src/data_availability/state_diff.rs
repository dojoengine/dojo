//! Formats the starknet state diff to be published
//! on a DA layer.
//!
//! All the specification is available here:
//! https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/on-chain-data
//!
//! We use `U256` from ethers for easier computation (than working with felts).
//!
//! Optims:
//! Currently, the serialize functions are using `iter().find()` on arrays
//! to know if an address has been deployed or declared.
//! To avoid this overhead, we may want to first generate an hashmap of such
//! arrays to then have O(1) search.
use ethers::types::U256;
use starknet::core::types::{FieldElement, StateDiff, ContractStorageDiffItem, NonceUpdate, DeployedContractItem, DeclaredClassItem};
use std::collections::HashSet;

// 2 ^ 128
const CLASS_INFO_FLAG_TRUE: &str = "0x100000000000000000000000000000000";

/// Converts the [`StateDiff`] from RPC types into a [`Vec<FieldElement>`].
///
/// Currently, Katana does not support `replaced_classes` and `deprecated_declared_classes`:
/// https://github.com/dojoengine/dojo/blob/10031f0abba7ca8dafc7040a52883e5af469863a/crates/katana/rpc/rpc-types/src/state_update.rs#L66.
///
/// For this reason, the [`StateDiff`] serialized here does not take in account
/// the contracts that has only been upgraded via `replace_class` syscall.
///
/// # Arguments
///
/// * `state_diff` - The [`StateDiff`] to serialize.
pub fn state_diff_to_felts(state_diff: &StateDiff) -> Vec<FieldElement> {
    let mut data = vec![];

    // Order matters here, storage then nonce then deployed.
    let mut processed_addresses = serialize_storage_updates(state_diff, &mut data);
    serialize_nonce_updates(state_diff, &mut processed_addresses, &mut data);
    serialize_deployed_updates(state_diff, &mut processed_addresses, &mut data);

    // Iterate over all the declared classes.
    data.push(state_diff.declared_classes.len().into());

    for decl in &state_diff.declared_classes {
        let DeclaredClassItem { class_hash, compiled_class_hash } = decl;
        data.push(*class_hash);
        data.push(*compiled_class_hash);
    }

    data.insert(0, processed_addresses.len().into());

    data
}

/// Serializes for the DA all the contracts that have at least one storage update.
/// This may include nonce and/or deployed-declared updates.
/// Returns a [`HashSet<FieldElement>`] of all the contract addresses that
/// were uniquely processed.
///
/// # Arguments
///
/// * `state_diff` - The state diff to process.
/// * `out` - The output buffer to serialize into.
fn serialize_storage_updates(state_diff: &StateDiff, out: &mut Vec<FieldElement>) -> HashSet<FieldElement> {
    let mut processed_addresses = HashSet::new();

    for contract_diff in &state_diff.storage_diffs {
        let ContractStorageDiffItem { address: addr, storage_entries: entries } = contract_diff;

        processed_addresses.insert(*addr);

        let deployed_replaced = state_diff
            .deployed_contracts
            .iter()
            .find(|c| c.address == *addr);
        // Currently, Katana does not populate the replaced_classes.
        //.or_else(|| state_diff.replaced_classes.get(&addr));

        out.push(*addr);

        out.push(compute_update_meta_info(
            state_diff.nonces.iter().find(|c| c.contract_address == *addr).map(|c| c.nonce),
            entries.len() as u64,
            deployed_replaced.is_some(),
        ));

        if let Some(c) = deployed_replaced {
            out.push(c.class_hash);
        }

        for e in entries {
            out.push(e.key);
            out.push(e.value);
        }
    }

    processed_addresses
}

/// Serializes for the DA all the contracts that have at least one nonce update.
/// This may include deployed-declared updates.
///
/// # Arguments
///
/// * `state_diff` - The state diff to process.
/// * `processed_addresses` - A list of already processed addresses to avoid duplication.
/// * `out` - The output buffer to serialize into.
fn serialize_nonce_updates(state_diff: &StateDiff, processed_addresses: &mut HashSet<FieldElement>, out: &mut Vec<FieldElement>) {
    for nonce_update in &state_diff.nonces {
        let NonceUpdate { contract_address: addr, nonce: new_nonce } = *nonce_update;

        if !processed_addresses.insert(addr) {
            continue;
        }

        let deployed_replaced = state_diff
            .deployed_contracts
            .iter()
            .find(|c| c.address == addr);
        // Currently, Katana does not populate the replaced_classes.
        //.or_else(|| state_diff.replaced_classes.get(&addr));

        out.push(addr);

        out.push(compute_update_meta_info(
            Some(new_nonce),
            0,
            deployed_replaced.is_some(),
        ));
    }
}

/// Serializes for the DA all the contracts that have been deployed
/// or their class hash replaced only.
///
/// # Arguments
///
/// * `state_diff` - The state diff to process.
/// * `processed_addresses` - A list of already processed addresses to avoid duplication.
/// * `out` - The output buffer to serialize into.
fn serialize_deployed_updates(state_diff: &StateDiff, processed_addresses: &mut HashSet<FieldElement>, out: &mut Vec<FieldElement>) {
    for deployed in &state_diff.deployed_contracts {
        let DeployedContractItem { address: addr, .. } = *deployed;

        if !processed_addresses.insert(addr) {
            continue;
        }

        out.push(addr);

        out.push(compute_update_meta_info(None, 0, true));
    }
}

/// Formats the contract meta information.
///
/// |---padding---|---class info flag---|---new nonce---|---# storage updates---|
///     127 bits          1 bit              64 bits             64 bits
///
/// # Arguments
///
/// * `new_nonce` - The new nonce for the contract address, None otherwise.
/// * `n_storage_updates` - The count of storage updates for the contract address.
/// * `is_storage_only` - True if the contract address was only modified
///   with storage updates. False if the contract was deployed or it's class hash
///   replaced during this state update.
fn compute_update_meta_info(new_nonce: Option<FieldElement>, n_storage_updates: u64, is_storage_only: bool) -> FieldElement {
    let mut meta = if is_storage_only {
        U256::from(0)
    } else {
        U256::from_str_radix(CLASS_INFO_FLAG_TRUE, 16).unwrap()
    };

    if let Some(nonce) = new_nonce {
        // At the moment, v0.11 and forward are packing the nonce into 64 bits.
        let nonce_u64: u64 = nonce.try_into().expect("Nonce too large for DA serialization");
        meta += ((nonce_u64 as u128) << 64).into()
    }

    meta += (n_storage_updates as u128).into();

    FieldElement::from_hex_be(format!("0x{:064x}", meta).as_str()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_update_meta_info_no_flag() {
        let info = compute_update_meta_info(Some(FieldElement::ONE), 1, true);
        assert_eq!(info, FieldElement::from_hex_be("0x00000000000000010000000000000001").unwrap());
    }

    #[test]
    fn compute_update_meta_info_with_flag() {
        let info = compute_update_meta_info(Some(FieldElement::ONE), 1, false);
        assert_eq!(info, FieldElement::from_hex_be("0x100000000000000010000000000000001").unwrap());
    }
}
