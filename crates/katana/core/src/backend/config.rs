use blockifier::block_context::{BlockContext, FeeTokenAddresses, GasPrices};
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::ChainId;
use url::Url;

use crate::constants::{
    DEFAULT_GAS_PRICE, DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS, FEE_TOKEN_ADDRESS,
    SEQUENCER_ADDRESS,
};
use crate::env::{get_default_vm_resource_fee_cost, BlockContextGenerator};

#[derive(Debug, Clone)]
pub struct StarknetConfig {
    pub seed: [u8; 32],
    pub total_accounts: u8,
    pub disable_fee: bool,
    pub env: Environment,
    pub fork_rpc_url: Option<Url>,
    pub fork_block_number: Option<u64>,
    pub disable_validate: bool,
}

impl StarknetConfig {
    pub fn block_context(&self) -> BlockContext {
        BlockContext {
            block_number: BlockNumber::default(),
            chain_id: ChainId(self.env.chain_id.clone()),
            block_timestamp: BlockTimestamp::default(),
            sequencer_address: (*SEQUENCER_ADDRESS).into(),
            // As the fee has two currencies, we also have to adjust their addresses.
            // https://github.com/starkware-libs/blockifier/blob/51b343fe38139a309a69b2482f4b484e8caa5edf/crates/blockifier/src/block_context.rs#L34
            fee_token_addresses: FeeTokenAddresses {
                eth_fee_token_address: (*FEE_TOKEN_ADDRESS).into(),
                strk_fee_token_address: Default::default(),
            },
            vm_resource_fee_cost: get_default_vm_resource_fee_cost().into(),
            // Gas prices are dual too.
            // https://github.com/starkware-libs/blockifier/blob/51b343fe38139a309a69b2482f4b484e8caa5edf/crates/blockifier/src/block_context.rs#L49
            gas_prices: GasPrices {
                eth_l1_gas_price: self.env.gas_price,
                strk_l1_gas_price: Default::default(),
            },
            validate_max_n_steps: self.env.validate_max_steps,
            invoke_tx_max_n_steps: self.env.invoke_max_steps,
            max_recursion_depth: 1000,
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
            total_accounts: 10,
            disable_fee: false,
            fork_rpc_url: None,
            fork_block_number: None,
            env: Environment::default(),
            disable_validate: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub chain_id: String,
    pub gas_price: u128,
    pub invoke_max_steps: u32,
    pub validate_max_steps: u32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            gas_price: DEFAULT_GAS_PRICE,
            chain_id: "KATANA".to_string(),
            invoke_max_steps: DEFAULT_INVOKE_MAX_STEPS,
            validate_max_steps: DEFAULT_VALIDATE_MAX_STEPS,
        }
    }
}
