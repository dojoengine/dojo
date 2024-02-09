use katana_primitives::block::{BlockNumber, SealedBlock};
use katana_primitives::state::StateUpdatesWithDeclaredClasses;

use crate::ProviderResult;

#[async_trait::async_trait]
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait Provider {
    /// Fetches the current block number of underlying chain.
    async fn block_number(&self) -> ProviderResult<BlockNumber>;

    /// Fetches a block with it's transactions.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block to fetch.
    async fn fetch_block(&self, block_number: BlockNumber) -> ProviderResult<SealedBlock>;

    /// Fetches the state updates related to a given block.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block to fetch.
    async fn fetch_state_updates(
        &self,
        block_number: BlockNumber,
    ) -> ProviderResult<StateUpdatesWithDeclaredClasses>;
}
