use alloy_primitives::U256;

use crate::chain::ChainId;
use crate::genesis::allocation::DevAllocationsGenerator;
use crate::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use crate::genesis::Genesis;

/// A chain specification.
#[derive(Debug, Clone)]
pub struct ChainSpec {
    /// The network chain id.
    pub id: ChainId,
    /// The genesis block.
    pub genesis: Genesis,
}

impl Default for ChainSpec {
    fn default() -> Self {
        let id = ChainId::parse("KATANA").unwrap();

        let accounts = DevAllocationsGenerator::new(10)
            .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
            .generate();

        let mut genesis = Genesis::default();
        genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));

        Self { id, genesis }
    }
}
