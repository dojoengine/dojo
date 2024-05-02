//! Starknet OS inputs.
//!
//! Python code:
//! <https://github.com/starkware-libs/cairo-lang/blob/caba294d82eeeccc3d86a158adb8ba209bf2d8fc/src/starkware/starknet/core/os/os_input.py#L29>
use katana_primitives::block::SealedBlock;
use snos::io::input::StarknetOsInput;

use super::{felt, transaction};

/// Setups a default [`StarknetOsInput`] with the block info.
///
/// TODO: currently no commitments are computed, but are required
/// to be in the [`StarknetOsInput`].
/// TODO: some of the input fields can be init from the state.
pub fn snos_input_from_block(block: &SealedBlock) -> StarknetOsInput {
    let transactions = block.body.iter().map(transaction::snos_internal_from_tx).collect();

    StarknetOsInput {
        transactions,
        block_hash: felt::from_ff(&block.header.hash),
        ..Default::default()
    }
}
