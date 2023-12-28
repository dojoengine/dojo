use std::collections::HashMap;

use blockifier::block_context::{BlockContext, FeeTokenAddresses, GasPrices};
use cairo_vm::vm::runners::builtin_runner::{
    BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
    OUTPUT_BUILTIN_NAME, POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME,
    SEGMENT_ARENA_BUILTIN_NAME, SIGNATURE_BUILTIN_NAME,
};
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::ChainId;

use crate::constants::{DEFAULT_GAS_PRICE, FEE_TOKEN_ADDRESS, SEQUENCER_ADDRESS};

/// Represents the chain environment.
#[derive(Debug, Clone)]
pub struct Env {
    /// The block environment of the current block. This is the context that
    /// the transactions will be executed on.
    pub block: BlockContext,
}

#[derive(Debug, Default)]
pub struct BlockContextGenerator {
    pub block_timestamp_offset: i64,
    pub next_block_start_time: u64,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            block: BlockContext {
                chain_id: ChainId("KATANA".to_string()),
                block_number: BlockNumber::default(),
                block_timestamp: BlockTimestamp::default(),
                sequencer_address: (*SEQUENCER_ADDRESS).into(),
                fee_token_addresses: FeeTokenAddresses {
                    eth_fee_token_address: (*FEE_TOKEN_ADDRESS).into(),
                    strk_fee_token_address: Default::default(),
                },
                vm_resource_fee_cost: get_default_vm_resource_fee_cost().into(),
                gas_prices: GasPrices {
                    eth_l1_gas_price: DEFAULT_GAS_PRICE,
                    strk_l1_gas_price: Default::default(),
                },
                invoke_tx_max_n_steps: 1_000_000,
                validate_max_n_steps: 1_000_000,
                max_recursion_depth: 100,
            },
        }
    }
}

pub fn get_default_vm_resource_fee_cost() -> HashMap<String, f64> {
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
