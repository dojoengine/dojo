use std::path::PathBuf;

use blockifier::block_context::BlockContext;
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::{ChainId, ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;

use crate::block_context::{get_default_vm_resource_fee_cost, BlockContextGenerator};
use crate::constants::{DEFAULT_GAS_PRICE, FEE_TOKEN_ADDRESS, SEQUENCER_ADDRESS};

#[derive(Debug)]
pub struct StarknetConfig {
    pub seed: [u8; 32],
    pub auto_mine: bool,
    pub total_accounts: u8,
    pub allow_zero_max_fee: bool,
    pub account_path: Option<PathBuf>,
    pub env: Environment,
}

impl StarknetConfig {
    pub fn block_context(&self) -> BlockContext {
        BlockContext {
            block_number: BlockNumber::default(),
            chain_id: ChainId(self.env.chain_id.clone()),
            block_timestamp: BlockTimestamp::default(),
            sequencer_address: ContractAddress(patricia_key!(*SEQUENCER_ADDRESS)),
            fee_token_address: ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS)),
            vm_resource_fee_cost: get_default_vm_resource_fee_cost(),
            gas_price: self.env.gas_price,
            validate_max_n_steps: 1_000_000,
            invoke_tx_max_n_steps: 1_000_000,
        }
    }

    pub fn block_context_generator(&self) -> BlockContextGenerator {
        BlockContextGenerator::default()
    }
}

impl Default for StarknetConfig {
    fn default() -> Self {
        Self {
            seed: [0; 32],
            auto_mine: true,
            total_accounts: 10,
            account_path: None,
            allow_zero_max_fee: false,
            env: Environment::default(),
        }
    }
}

#[derive(Debug)]
pub struct Environment {
    pub chain_id: String,
    pub gas_price: u128,
}

impl Default for Environment {
    fn default() -> Self {
        Self { chain_id: "KATANA".to_string(), gas_price: DEFAULT_GAS_PRICE }
    }
}
