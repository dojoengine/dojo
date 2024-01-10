use std::collections::HashMap;
use std::time::SystemTime;

use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};

use crate::constants::{
    ERC20_CONTRACT, ERC20_CONTRACT_CLASS_HASH, ERC20_CONTRACT_COMPILED_CLASS_HASH,
    ERC20_DECIMALS_STORAGE_SLOT, ERC20_NAME_STORAGE_SLOT, ERC20_SYMBOL_STORAGE_SLOT,
    FEE_TOKEN_ADDRESS, OZ_V1_ACCOUNT_CONTRACT, OZ_V1_ACCOUNT_CONTRACT_CLASS_HASH,
    OZ_V1_ACCOUNT_CONTRACT_COMPILED, OZ_V1_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH, UDC_ADDRESS,
    UDC_CLASS_HASH, UDC_COMPILED_CLASS_HASH, UDC_CONTRACT,
};

pub(super) fn get_current_timestamp() -> std::time::Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("should get current UNIX timestamp")
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
        (*OZ_V1_ACCOUNT_CONTRACT_CLASS_HASH, *OZ_V1_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH),
    ]);

    let declared_sierra_classes = HashMap::from([(
        *OZ_V1_ACCOUNT_CONTRACT_CLASS_HASH,
        OZ_V1_ACCOUNT_CONTRACT.clone().flatten().unwrap(),
    )]);

    let declared_compiled_classes = HashMap::from([
        (*UDC_CLASS_HASH, (*UDC_CONTRACT).clone()),
        (*ERC20_CONTRACT_CLASS_HASH, (*ERC20_CONTRACT).clone()),
        (*OZ_V1_ACCOUNT_CONTRACT_CLASS_HASH, (*OZ_V1_ACCOUNT_CONTRACT_COMPILED).clone()),
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
