use std::path::PathBuf;

use alloy_primitives::U256;
use katana_primitives::chain::ChainId;
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use katana_primitives::genesis::Genesis;
use url::Url;

use crate::constants::{DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS};
use crate::env::BlockContextGenerator;

#[derive(Debug, Clone)]
pub struct StarknetConfig {
    pub disable_fee: bool,
    pub env: Environment,
    pub fork_rpc_url: Option<Url>,
    pub fork_block_number: Option<u64>,
    pub disable_validate: bool,
    pub db_dir: Option<PathBuf>,
    pub genesis: Genesis,
}

impl StarknetConfig {
    pub fn block_context_generator(&self) -> BlockContextGenerator {
        BlockContextGenerator::default()
    }
}

impl Default for StarknetConfig {
    fn default() -> Self {
        let accounts = DevAllocationsGenerator::new(10)
            .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
            .generate();

        let mut genesis = Genesis::default();
        genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));

        Self {
            disable_fee: false,
            fork_rpc_url: None,
            fork_block_number: None,
            env: Environment::default(),
            disable_validate: false,
            db_dir: None,
            genesis,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub chain_id: ChainId,
    pub invoke_max_steps: u32,
    pub validate_max_steps: u32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            chain_id: ChainId::parse("KATANA").unwrap(),
            invoke_max_steps: DEFAULT_INVOKE_MAX_STEPS,
            validate_max_steps: DEFAULT_VALIDATE_MAX_STEPS,
        }
    }
}
