use katana_primitives::block::GasPrices;
use katana_primitives::env::{BlockEnv, CfgEnv, FeeTokenAddressses};
use katana_primitives::genesis::constant::DEFAULT_FEE_TOKEN_ADDRESS;
use katana_primitives::transaction::{ExecutableTxWithHash, InvokeTx, InvokeTxV1};
use katana_primitives::Felt;
use starknet::macros::{felt, selector};

pub fn tx() -> ExecutableTxWithHash {
    let invoke = InvokeTx::V1(InvokeTxV1 {
        sender_address: felt!("0x1").into(),
        calldata: vec![
            DEFAULT_FEE_TOKEN_ADDRESS.into(),
            selector!("transfer"),
            Felt::THREE,
            felt!("0x100"),
            Felt::ONE,
            Felt::ZERO,
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
        fee_token_addresses: FeeTokenAddressses {
            eth: DEFAULT_FEE_TOKEN_ADDRESS,
            strk: DEFAULT_FEE_TOKEN_ADDRESS,
        },
        ..Default::default()
    };

    (block, cfg)
}
