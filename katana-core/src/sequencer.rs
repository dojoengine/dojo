use std::collections::HashMap;

use blockifier::{
    block_context::BlockContext,
    test_utils::{DEFAULT_GAS_PRICE, TEST_ERC20_CONTRACT_ADDRESS, TEST_SEQUENCER_ADDRESS},
};
use starknet_api::{
    block::{BlockNumber, BlockTimestamp},
    core::{ChainId, ContractAddress, PatriciaKey},
    hash::StarkHash,
    patricia_key,
};

use crate::state::DictStateReader;

pub struct Sequencer {
    pub block_context: BlockContext,
    pub state: DictStateReader,
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            block_context: BlockContext {
                chain_id: ChainId("SN_GOERLI".to_string()),
                block_number: BlockNumber::default(),
                block_timestamp: BlockTimestamp::default(),
                sequencer_address: ContractAddress(patricia_key!(TEST_SEQUENCER_ADDRESS)),
                fee_token_address: ContractAddress(patricia_key!(TEST_ERC20_CONTRACT_ADDRESS)),
                cairo_resource_fee_weights: HashMap::from([
                    (String::from("n_steps"), 1_f64),
                    (String::from("pedersen_builtin"), 1_f64),
                    (String::from("range_check_builtin"), 1_f64),
                    (String::from("ecdsa_builtin"), 1_f64),
                    (String::from("bitwise_builtin"), 1_f64),
                    (String::from("poseidon_builtin"), 1_f64),
                    (String::from("output_builtin"), 1_f64),
                    (String::from("ec_op_builtin"), 1_f64),
                ]),
                gas_price: DEFAULT_GAS_PRICE,
                invoke_tx_max_n_steps: 1_000_000,
                validate_max_n_steps: 1_000_000,
            },
            state: DictStateReader::default(),
        }
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}
