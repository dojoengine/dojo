pub mod contract;
pub mod event;

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use blockifier::state::cached_state::CommitmentStateDiff;
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, NonceUpdate, StateDiff,
    StorageEntry,
};
use starknet_api::hash::StarkFelt;
use starknet_api::StarknetApiError;

use crate::constants::{
    ERC20_CONTRACT, ERC20_CONTRACT_CLASS_HASH, ERC20_CONTRACT_COMPILED_CLASS_HASH,
    ERC20_DECIMALS_STORAGE_SLOT, ERC20_NAME_STORAGE_SLOT, ERC20_SYMBOL_STORAGE_SLOT,
    FEE_TOKEN_ADDRESS, OZ_V0_ACCOUNT_CONTRACT, OZ_V0_ACCOUNT_CONTRACT_CLASS_HASH,
    OZ_V0_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH, UDC_ADDRESS, UDC_CLASS_HASH,
    UDC_COMPILED_CLASS_HASH, UDC_CONTRACT,
};

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

pub(super) fn get_genesis_states_for_testing() -> StateUpdatesWithDeclaredClasses {
    let nonce_updates =
        HashMap::from([(*UDC_ADDRESS, 0u8.into()), (*FEE_TOKEN_ADDRESS, 0u8.into())]);

    let storage_updates = HashMap::from([(
        *FEE_TOKEN_ADDRESS,
        HashMap::from([
            (*ERC20_DECIMALS_STORAGE_SLOT, 18_u128.into()),
            (*ERC20_SYMBOL_STORAGE_SLOT, 0x455448_u128.into()),
            (*ERC20_NAME_STORAGE_SLOT, 0x4574686572_u128.into()),
        ]),
    )]);

    let contract_updates = HashMap::from([
        (*UDC_ADDRESS, *UDC_CLASS_HASH),
        (*FEE_TOKEN_ADDRESS, *ERC20_CONTRACT_CLASS_HASH),
    ]);

    let declared_classes = HashMap::from([
        (*UDC_CLASS_HASH, *UDC_COMPILED_CLASS_HASH),
        (*ERC20_CONTRACT_CLASS_HASH, *ERC20_CONTRACT_COMPILED_CLASS_HASH),
        (*OZ_V0_ACCOUNT_CONTRACT_CLASS_HASH, *OZ_V0_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH),
    ]);

    let declared_sierra_classes = HashMap::from([]);

    let declared_compiled_classes = HashMap::from([
        (*UDC_COMPILED_CLASS_HASH, (*UDC_CONTRACT).clone()),
        (*ERC20_CONTRACT_COMPILED_CLASS_HASH, (*ERC20_CONTRACT).clone()),
        (*OZ_V0_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH, (*OZ_V0_ACCOUNT_CONTRACT).clone()),
    ]);

    StateUpdatesWithDeclaredClasses {
        declared_sierra_classes,
        declared_compiled_classes,
        state_updates: StateUpdates {
            nonce_updates,
            storage_updates,
            contract_updates,
            declared_classes,
        },
    }
}
