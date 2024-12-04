use katana_primitives::block::BlockNumber;

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StageCheckpointProvider: Send + Sync {
    /// Returns the number of the last block that was successfully processed by the stage.
    fn checkpoint(&self, id: &str) -> ProviderResult<Option<BlockNumber>>;

    /// Sets the checkpoint for a stage to the given block number.
    fn set_checkpoint(&self, id: &str, block_number: BlockNumber) -> ProviderResult<()>;
}
