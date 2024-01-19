use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::state::StateUpdates;

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateUpdateProvider: Send + Sync {
    /// Returns the state update at the given block.
    fn state_update(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<StateUpdates>>;
}
