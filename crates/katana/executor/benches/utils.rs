use std::collections::HashMap;

use katana_cairo::cairo_vm::vm::runners::builtin_runner::{
    BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
    OUTPUT_BUILTIN_NAME, POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME,
    SEGMENT_ARENA_BUILTIN_NAME, SIGNATURE_BUILTIN_NAME,
};
use katana_primitives::block::GasPrices;
use katana_primitives::env::{BlockEnv, CfgEnv, FeeTokenAddressses};
use katana_primitives::genesis::constant::DEFAULT_FEE_TOKEN_ADDRESS;
use katana_primitives::transaction::{ExecutableTxWithHash, InvokeTx, InvokeTxV1};
use katana_primitives::FieldElement;
use starknet::macros::{felt, selector};

pub fn tx() -> ExecutableTxWithHash {
    let invoke = InvokeTx::V1(InvokeTxV1 {
        sender_address: felt!("0x1").into(),
        calldata: vec![
            DEFAULT_FEE_TOKEN_ADDRESS.into(),
            selector!("transfer"),
            FieldElement::THREE,
            felt!("0x100"),
            FieldElement::ONE,
            FieldElement::ZERO,
        ],
        max_fee: 10_000,
        ..Default::default()
    });

    ExecutableTxWithHash::new(invoke.into())
}

pub fn envs() -> (BlockEnv, CfgEnv) {
    let block = BlockEnv {
        l1_gas_prices: GasPrices { eth: 1, strk: 1 },
        sequencer_address: felt!("0x1337").into(),
        ..Default::default()
    };
    let cfg = CfgEnv {
        max_recursion_depth: 100,
        validate_max_n_steps: 4_000_000,
        invoke_tx_max_n_steps: 4_000_000,
        vm_resource_fee_cost: vm_resource_fee_cost(),
        fee_token_addresses: FeeTokenAddressses {
            eth: DEFAULT_FEE_TOKEN_ADDRESS,
            strk: DEFAULT_FEE_TOKEN_ADDRESS,
        },
        ..Default::default()
    };

    (block, cfg)
}

fn vm_resource_fee_cost() -> HashMap<String, f64> {
    HashMap::from([
        (String::from("n_steps"), 1_f64),
        (HASH_BUILTIN_NAME.to_string(), 1_f64),
        (RANGE_CHECK_BUILTIN_NAME.to_string(), 1_f64),
        (SIGNATURE_BUILTIN_NAME.to_string(), 1_f64),
        (BITWISE_BUILTIN_NAME.to_string(), 1_f64),
        (POSEIDON_BUILTIN_NAME.to_string(), 1_f64),
        (OUTPUT_BUILTIN_NAME.to_string(), 1_f64),
        (EC_OP_BUILTIN_NAME.to_string(), 1_f64),
        (KECCAK_BUILTIN_NAME.to_string(), 1_f64),
        (SEGMENT_ARENA_BUILTIN_NAME.to_string(), 1_f64),
    ])
}
