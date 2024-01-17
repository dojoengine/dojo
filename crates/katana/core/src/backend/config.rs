use std::path::PathBuf;

use katana_primitives::block::GasPrices;
use katana_primitives::chain::ChainId;
use katana_primitives::env::BlockEnv;
use url::Url;

use crate::constants::{DEFAULT_GAS_PRICE, DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS};
use crate::env::BlockContextGenerator;

#[derive(Debug, Clone)]
pub struct StarknetConfig {
    pub seed: [u8; 32],
    pub total_accounts: u8,
    pub disable_fee: bool,
    pub env: Environment,
    pub fork_rpc_url: Option<Url>,
    pub fork_block_number: Option<u64>,
    pub disable_validate: bool,
    pub db_dir: Option<PathBuf>,
}

impl StarknetConfig {
    pub fn block_env(&self) -> BlockEnv {
        BlockEnv {
            l1_gas_prices: GasPrices { eth: self.env.gas_price, ..Default::default() },
            ..Default::default()
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
            db_dir: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub chain_id: ChainId,
    pub gas_price: u64,
    pub invoke_max_steps: u32,
    pub validate_max_steps: u32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            gas_price: DEFAULT_GAS_PRICE,
            chain_id: ChainId::parse("KATANA").unwrap(),
            invoke_max_steps: DEFAULT_INVOKE_MAX_STEPS,
            validate_max_steps: DEFAULT_VALIDATE_MAX_STEPS,
        }
    }
}
