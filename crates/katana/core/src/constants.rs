use katana_primitives::contract::ContractAddress;
use lazy_static::lazy_static;
use starknet::macros::felt;

pub const DEFAULT_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of wei.

pub const DEFAULT_INVOKE_MAX_STEPS: u32 = 1_000_000;
pub const DEFAULT_VALIDATE_MAX_STEPS: u32 = 1_000_000;

pub const MAX_RECURSION_DEPTH: usize = 1000;

lazy_static! {

    // Predefined contract addresses

    pub static ref SEQUENCER_ADDRESS: ContractAddress = ContractAddress(felt!("0x1"));

}
