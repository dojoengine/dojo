use anyhow::Result;
use katana_primitives::block::{BlockHashOrNumber, StateUpdate};

pub trait StateUpdateProvider {
    /// Returns the state update for the given block.
    fn state_update(&self, block_id: BlockHashOrNumber) -> Result<Option<StateUpdate>>;
}
