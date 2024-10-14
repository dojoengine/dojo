use alloy_primitives::U256;
use lazy_static::lazy_static;

use crate::chain::ChainId;
use crate::contract::ContractAddress;
use crate::genesis::allocation::DevAllocationsGenerator;
use crate::genesis::constant::{
    DEFAULT_ETH_FEE_TOKEN_ADDRESS, DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
    DEFAULT_STRK_FEE_TOKEN_ADDRESS,
};
use crate::genesis::Genesis;

/// A chain specification.
#[derive(Debug, Clone)]
pub struct ChainSpec {
    /// The network chain id.
    pub id: ChainId,
    /// The genesis block.
    pub genesis: Genesis,
    /// The chain fee token contract.
    pub fee_contracts: StarknetFeeContracts,
}

/// Tokens that can be used for transaction fee payments in the chain. As
/// supported on Starknet.
#[derive(Debug, Clone)]
pub struct StarknetFeeContracts {
    /// ETH fee token address. Used for paying pre-V3 transactions.
    pub eth: ContractAddress,
    /// STRK fee token address. Used for paying V3 transactions.
    pub strk: ContractAddress,
}

impl Default for ChainSpec {
    fn default() -> Self {
        DEV.clone()
    }
}

lazy_static! {
    /// The default chain specification in dev mode.
    pub static ref DEV: ChainSpec = {
        let mut chain_spec = DEV_UNALLOCATED.clone();

        let accounts = DevAllocationsGenerator::new(10)
            .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
            .generate();

        chain_spec.genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
        chain_spec
    };

    /// The default chain specification for dev mode but without any allocations.
    ///
    /// Used when we want to create a chain spec with user defined # of allocations.
    pub static ref DEV_UNALLOCATED: ChainSpec = {
        let id = ChainId::SEPOLIA;
        let genesis = Genesis::default();
        let fee_contracts = StarknetFeeContracts { eth: DEFAULT_ETH_FEE_TOKEN_ADDRESS, strk: DEFAULT_STRK_FEE_TOKEN_ADDRESS };
        ChainSpec { id, genesis, fee_contracts }
    };
}
