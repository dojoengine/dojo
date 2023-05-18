use std::collections::HashMap;

use blockifier::block_context::BlockContext;
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::{ChainId, ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;

use crate::constants::{DEFAULT_GAS_PRICE, FEE_TOKEN_ADDRESS, SEQUENCER_ADDRESS};
use crate::starknet::StarknetConfig;

pub trait Base {
    fn base() -> Self;
}

impl Base for BlockContext {
    fn base() -> Self {
        BlockContext {
            chain_id: ChainId("KATANA".to_string()),
            block_number: BlockNumber::default(),
            block_timestamp: BlockTimestamp::default(),
            sequencer_address: ContractAddress(patricia_key!(*SEQUENCER_ADDRESS)),
            fee_token_address: ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS)),
            vm_resource_fee_cost: HashMap::from([
                (String::from("n_steps"), 1_f64),
                (String::from("pedersen"), 1_f64),
                (String::from("range_check"), 1_f64),
                (String::from("ecdsa"), 1_f64),
                (String::from("bitwise"), 1_f64),
                (String::from("poseidon"), 1_f64),
                (String::from("output"), 1_f64),
                (String::from("ec_op"), 1_f64),
            ]),
            gas_price: DEFAULT_GAS_PRICE,
            invoke_tx_max_n_steps: 1_000_000,
            validate_max_n_steps: 1_000_000,
        }
    }
}

pub fn block_context_from_config(config: &StarknetConfig) -> BlockContext {
    BlockContext {
        block_number: BlockNumber::default(),
        chain_id: ChainId(config.chain_id.clone()),
        block_timestamp: BlockTimestamp::default(),
        sequencer_address: ContractAddress(patricia_key!(*SEQUENCER_ADDRESS)),
        fee_token_address: ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS)),
        vm_resource_fee_cost: HashMap::from([
            (String::from("n_steps"), 1_f64),
            (String::from("pedersen"), 1_f64),
            (String::from("range_check"), 1_f64),
            (String::from("ecdsa"), 1_f64),
            (String::from("bitwise"), 1_f64),
            (String::from("poseidon"), 1_f64),
            (String::from("output"), 1_f64),
            (String::from("ec_op"), 1_f64),
        ]),
        gas_price: config.gas_price,
        validate_max_n_steps: 1_000_000,
        invoke_tx_max_n_steps: 1_000_000,
    }
}
