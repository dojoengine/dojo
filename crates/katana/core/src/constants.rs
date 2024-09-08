use katana_primitives::contract::ContractAddress;
use lazy_static::lazy_static;
use starknet::macros::felt;

// Default gas prices
pub const DEFAULT_ETH_L1_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of Wei.
pub const DEFAULT_STRK_L1_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of STRK.

pub const DEFAULT_INVOKE_MAX_STEPS: u32 = 10_000_000;
pub const DEFAULT_VALIDATE_MAX_STEPS: u32 = 1_000_000;

pub const MAX_RECURSION_DEPTH: usize = 1000;

lazy_static! {

    // Predefined contract addresses

    pub static ref DEFAULT_SEQUENCER_ADDRESS: ContractAddress = ContractAddress(felt!("0x1"));

}
