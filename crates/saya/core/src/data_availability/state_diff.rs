//! Formats the starknet state diff to be published
//! on a DA layer.
//!
//! All the specification is available here:
//! <https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/on-chain-data>.
//!
//! We use `U256` from ethers for easier computation (than working with felts).
//!
//! Optims:
//! Currently, the serialize functions are using `iter().find()` on arrays
//! to know if an address has been deployed or declared.
//! To avoid this overhead, we may want to first generate an hashmap of such
//! arrays to then have O(1) search.
use std::collections::HashSet;

use ethers::types::U256;
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, FieldElement, NonceUpdate,
    StateDiff,
};

// 2 ^ 128
const CLASS_INFO_FLAG_TRUE: &str = "0x100000000000000000000000000000000";

/// Converts the [`StateDiff`] from RPC types into a [`Vec<FieldElement>`].
///
/// Currently, Katana does not support `replaced_classes` and `deprecated_declared_classes`:
/// <https://github.com/dojoengine/dojo/blob/10031f0abba7ca8dafc7040a52883e5af469863a/crates/katana/rpc/rpc-types/src/state_update.rs#L66>.
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
fn serialize_storage_updates(
    state_diff: &StateDiff,
    out: &mut Vec<FieldElement>,
) -> HashSet<FieldElement> {
    let mut processed_addresses = HashSet::new();

    for contract_diff in &state_diff.storage_diffs {
        let ContractStorageDiffItem { address, storage_entries: entries } = contract_diff;

        processed_addresses.insert(*address);

        let deployed_replaced =
            state_diff.deployed_contracts.iter().find(|c| c.address == *address);
        // Currently, Katana does not populate the replaced_classes.
        //.or_else(|| state_diff.replaced_classes.get(&addr));

        out.push(*address);

        out.push(compute_update_meta_info(
            state_diff.nonces.iter().find(|c| c.contract_address == *address).map(|c| c.nonce),
            entries.len() as u64,
            deployed_replaced.is_none(),
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
fn serialize_nonce_updates(
    state_diff: &StateDiff,
    processed_addresses: &mut HashSet<FieldElement>,
    out: &mut Vec<FieldElement>,
) {
    for nonce_update in &state_diff.nonces {
        let NonceUpdate { contract_address, nonce: new_nonce } = *nonce_update;

        if !processed_addresses.insert(contract_address) {
            continue;
        }

        let deployed_replaced =
            state_diff.deployed_contracts.iter().find(|c| c.address == contract_address);
        // Currently, Katana does not populate the replaced_classes.
        //.or_else(|| state_diff.replaced_classes.get(&addr));

        out.push(contract_address);

        out.push(compute_update_meta_info(Some(new_nonce), 0, deployed_replaced.is_none()));

        if let Some(c) = deployed_replaced {
            out.push(c.class_hash);
        }
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
fn serialize_deployed_updates(
    state_diff: &StateDiff,
    processed_addresses: &mut HashSet<FieldElement>,
    out: &mut Vec<FieldElement>,
) {
    for deployed in &state_diff.deployed_contracts {
        let DeployedContractItem { address, class_hash } = *deployed;

        if !processed_addresses.insert(address) {
            continue;
        }

        out.push(address);
        out.push(compute_update_meta_info(None, 0, false));
        out.push(class_hash);
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
/// * `is_storage_only` - True if the contract address was only modified with storage updates. False
///   if the contract was deployed or it's class hash replaced during this state update.
fn compute_update_meta_info(
    new_nonce: Option<FieldElement>,
    n_storage_updates: u64,
    is_storage_only: bool,
) -> FieldElement {
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
    use starknet::core::types::StorageEntry;
    use starknet::macros::{felt, selector};

    use super::*;

    #[test]
    fn compute_update_meta_info_no_flag() {
        let info = compute_update_meta_info(Some(FieldElement::ONE), 1, true);
        assert_eq!(info, felt!("0x00000000000000010000000000000001"));
    }

    #[test]
    fn compute_update_meta_info_with_flag() {
        let info = compute_update_meta_info(Some(FieldElement::ONE), 1, false);
        assert_eq!(info, felt!("0x100000000000000010000000000000001"));
    }

    #[test]
    fn serialize_storage_updates_only() {
        let contract_addr = selector!("addr1");

        let sd = StateDiff {
            storage_diffs: vec![ContractStorageDiffItem {
                address: contract_addr,
                storage_entries: vec![
                    StorageEntry { key: felt!("0x0"), value: felt!("0x1") },
                    StorageEntry { key: felt!("0xa"), value: felt!("0xb") },
                ],
            }],
            deprecated_declared_classes: Default::default(),
            declared_classes: Default::default(),
            deployed_contracts: Default::default(),
            replaced_classes: Default::default(),
            nonces: Default::default(),
        };

        let mut data = vec![];
        let addresses = serialize_storage_updates(&sd, &mut data);
        assert_eq!(addresses.len(), 1);
        assert_eq!(*addresses.get(&contract_addr).unwrap(), contract_addr);

        assert_eq!(data.len(), 6);
        assert_eq!(data[0], contract_addr);
        assert_eq!(data[1], felt!("0x2"));
        assert_eq!(data[2], felt!("0x0"));
        assert_eq!(data[3], felt!("0x1"));
        assert_eq!(data[4], felt!("0xa"));
        assert_eq!(data[5], felt!("0xb"));
    }

    #[test]
    fn serialize_nonce_updates_only() {
        let contract_address = selector!("account1");

        let sd = StateDiff {
            storage_diffs: Default::default(),
            deprecated_declared_classes: Default::default(),
            declared_classes: Default::default(),
            deployed_contracts: Default::default(),
            replaced_classes: Default::default(),
            nonces: vec![NonceUpdate { contract_address, nonce: felt!("0xff") }],
        };

        let mut data = vec![];
        let mut processed_addresses = HashSet::new();

        serialize_nonce_updates(&sd, &mut processed_addresses, &mut data);

        assert_eq!(data.len(), 2);
        assert_eq!(data[0], contract_address);
        assert_eq!(data[1], felt!("0x00000000000000ff0000000000000000"));
    }

    #[test]
    fn serialize_deployed_updates_only() {
        let address = selector!("addr1");
        let class_hash = selector!("classhash1");

        let sd = StateDiff {
            storage_diffs: Default::default(),
            deprecated_declared_classes: Default::default(),
            declared_classes: Default::default(),
            deployed_contracts: vec![DeployedContractItem { address, class_hash }],
            replaced_classes: Default::default(),
            nonces: Default::default(),
        };

        let mut data = vec![];
        let mut processed_addresses = HashSet::new();

        serialize_deployed_updates(&sd, &mut processed_addresses, &mut data);

        assert_eq!(data.len(), 3);
        assert_eq!(data[0], address);
        assert_eq!(data[1], felt!("0x100000000000000000000000000000000"));
        assert_eq!(data[2], class_hash);
    }

    #[test]
    fn state_diff_to_felts_full() {
        // Account 1: nonce update + storage updates + deployed + declared.
        let a1_addr = selector!("a1");
        let a1_ch = selector!("a1_ch");
        let a1_cch = selector!("a1_cch");
        let a1_nonce = felt!("0xf1");

        // Account 2: nonce update.
        let a2_addr = selector!("a2");
        let a2_nonce = felt!("0xf2");

        // Contract 1: storage updates + deployed + declared.
        let c1_addr = selector!("c1");
        let c1_ch = selector!("c1_ch");
        let c1_cch = selector!("c1_cch");

        // Contract 2: storage updates only.
        let c2_addr = selector!("c2");

        // Contract 3: only deployed and declared.
        let c3_addr = selector!("c3");
        let c3_ch = selector!("c3_ch");
        let c3_cch = selector!("c3_cch");

        // Contract 4: only declared.
        let c4_ch = selector!("c4_ch");
        let c4_cch = selector!("c4_cch");

        let sd = StateDiff {
            storage_diffs: vec![
                ContractStorageDiffItem {
                    address: a1_addr,
                    storage_entries: vec![StorageEntry { key: felt!("0x0"), value: felt!("0xa1") }],
                },
                ContractStorageDiffItem {
                    address: c1_addr,
                    storage_entries: vec![StorageEntry { key: felt!("0x0"), value: felt!("0xc1") }],
                },
                ContractStorageDiffItem {
                    address: c2_addr,
                    storage_entries: vec![
                        StorageEntry { key: felt!("0x0"), value: felt!("0xc2") },
                        StorageEntry { key: felt!("0x1"), value: felt!("0xc2") },
                    ],
                },
            ],
            deployed_contracts: vec![
                DeployedContractItem { address: a1_addr, class_hash: a1_ch },
                DeployedContractItem { address: c1_addr, class_hash: c1_ch },
                DeployedContractItem { address: c3_addr, class_hash: c3_ch },
            ],
            nonces: vec![
                NonceUpdate { contract_address: a1_addr, nonce: a1_nonce },
                NonceUpdate { contract_address: a2_addr, nonce: a2_nonce },
            ],
            declared_classes: vec![
                DeclaredClassItem { class_hash: a1_ch, compiled_class_hash: a1_cch },
                DeclaredClassItem { class_hash: c1_ch, compiled_class_hash: c1_cch },
                DeclaredClassItem { class_hash: c3_ch, compiled_class_hash: c3_cch },
                DeclaredClassItem { class_hash: c4_ch, compiled_class_hash: c4_cch },
            ],
            deprecated_declared_classes: Default::default(),
            replaced_classes: Default::default(),
        };

        let data = state_diff_to_felts(&sd);

        assert_eq!(data.len(), 31);

        // Only 5 contract updates, no duplication expected.
        assert_eq!(data[0], felt!("5"));

        // We follow the order of storage updates first, then nonce updates (that have no
        // storage update), then deployed updated and finally declare updates.

        // Storage updates (which may include other updates).
        assert_eq!(data[1], a1_addr);
        assert_eq!(data[2], felt!("0x100000000000000f10000000000000001"));
        assert_eq!(data[3], a1_ch);
        assert_eq!(data[4], felt!("0x0"));
        assert_eq!(data[5], felt!("0xa1"));

        assert_eq!(data[6], c1_addr);
        assert_eq!(data[7], felt!("0x100000000000000000000000000000001"));
        assert_eq!(data[8], c1_ch);
        assert_eq!(data[9], felt!("0x0"));
        assert_eq!(data[10], felt!("0xc1"));

        assert_eq!(data[11], c2_addr);
        assert_eq!(data[12], felt!("0x00000000000000000000000000000002"));
        assert_eq!(data[13], felt!("0x0"));
        assert_eq!(data[14], felt!("0xc2"));
        assert_eq!(data[15], felt!("0x1"));
        assert_eq!(data[16], felt!("0xc2"));

        // Nonce updates only.
        assert_eq!(data[17], a2_addr);
        assert_eq!(data[18], felt!("0x00000000000000f20000000000000000"));

        // Deployed only.
        assert_eq!(data[19], c3_addr);
        assert_eq!(data[20], felt!("0x100000000000000000000000000000000"));
        assert_eq!(data[21], c3_ch);

        // Declare updates.
        assert_eq!(data[22], felt!("4"));
        assert_eq!(data[23], a1_ch);
        assert_eq!(data[24], a1_cch);
        assert_eq!(data[25], c1_ch);
        assert_eq!(data[26], c1_cch);
        assert_eq!(data[27], c3_ch);
        assert_eq!(data[28], c3_cch);
        assert_eq!(data[29], c4_ch);
        assert_eq!(data[30], c4_cch);
    }
}
