use std::collections::HashMap;

use crate::block::{BlockNumber, GasPrices};
use crate::chain::ChainId;
use crate::contract::ContractAddress;

/// Block environment values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockEnv {
    /// The block height.
    pub number: BlockNumber,
    /// The timestamp in seconds since the UNIX epoch.
    pub timestamp: u64,
    /// The L1 gas prices at this particular block.
    pub l1_gas_prices: GasPrices,
    /// The contract address of the sequencer.
    pub sequencer_address: ContractAddress,
}

/// The chain block execution configuration values.
#[derive(Debug, Clone, Default)]
pub struct CfgEnv {
    /// The chain id.
    pub chain_id: ChainId,
    /// The contract addresses of the fee tokens.
    pub fee_token_addresses: FeeTokenAddressses,
    /// The fee cost of the VM resources.
    pub vm_resource_fee_cost: HashMap<String, f64>,
    /// The maximum number of steps allowed for an invoke transaction.
    pub invoke_tx_max_n_steps: u32,
    /// The maximum number of steps allowed for transaction validation.
    pub validate_max_n_steps: u32,
    /// The maximum recursion depth allowed.
    pub max_recursion_depth: usize,
}

/// The contract addresses of the tokens used for the fees.
#[derive(Debug, Clone, Default)]
pub struct FeeTokenAddressses {
    /// The contract address of the `STRK` token.
    pub strk: ContractAddress,
    /// The contract address of the `ETH` token.
    pub eth: ContractAddress,
}
