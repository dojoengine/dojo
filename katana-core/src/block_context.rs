use std::collections::HashMap;

use blockifier::block_context::BlockContext;
use starknet_api::{
    block::{BlockNumber, BlockTimestamp},
    core::{ChainId, ContractAddress, PatriciaKey},
    hash::StarkHash,
    patricia_key,
};

use crate::constants::{DEFAULT_GAS_PRICE, FEE_ERC20_CONTRACT_ADDRESS, SEQUENCER_ADDRESS};

pub trait Base {
    fn base() -> Self;
}

impl Base for BlockContext {
    fn base() -> Self {
        BlockContext {
            chain_id: ChainId("KATANA".to_string()),
            block_number: BlockNumber::default(),
            block_timestamp: BlockTimestamp::default(),
            sequencer_address: ContractAddress(patricia_key!(SEQUENCER_ADDRESS)),
            fee_token_address: ContractAddress(patricia_key!(FEE_ERC20_CONTRACT_ADDRESS)),
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
