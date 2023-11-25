use anyhow::Result;
use katana_primitives::block::{BlockHashOrNumber, StateUpdate};

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateUpdateProvider: Send + Sync {
    /// Returns the state update for the given block.
    fn state_update(&self, block_id: BlockHashOrNumber) -> Result<Option<StateUpdate>>;
}
