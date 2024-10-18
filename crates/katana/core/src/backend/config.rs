use crate::constants::{DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS};
use crate::env::BlockContextGenerator;

#[derive(Debug, Clone, Default)]
pub struct StarknetConfig {
    pub env: Environment,
}

impl StarknetConfig {
    pub fn block_context_generator(&self) -> BlockContextGenerator {
        BlockContextGenerator::default()
    }
}

// TODO: i think block limits should be included in chain spec
#[derive(Debug, Clone)]
pub struct Environment {
    pub invoke_max_steps: u32,
    pub validate_max_steps: u32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            invoke_max_steps: DEFAULT_INVOKE_MAX_STEPS,
            validate_max_steps: DEFAULT_VALIDATE_MAX_STEPS,
        }
    }
}
