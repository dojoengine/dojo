pub const DEFAULT_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of wei.
pub const SEQUENCER_ADDRESS: &str = "0x69";
pub const FEE_ERC20_CONTRACT_ADDRESS: &str = "0x420";

mod block_context;
pub mod sequencer;
pub mod state;
mod util;
