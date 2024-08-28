use katana_primitives::block::BlockNumber;

use crate::ProviderResult;

pub const SEND_FROM_BLOCK_KEY: u64 = 1;
pub const GATHER_FROM_BLOCK_KEY: u64 = 2;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait MessagingProvider: Send + Sync {
    /// Sets the send from block.
    fn set_send_from_block(&self, send_from_block: BlockNumber) -> ProviderResult<()>;
    /// Returns the send from block.
    fn get_send_from_block(&self) -> ProviderResult<Option<BlockNumber>>;
    /// Sets the gather from block.
    fn set_gather_from_block(&self, gather_from_block: BlockNumber) -> ProviderResult<()>;
    /// Returns the gather from block.
    fn get_gather_from_block(&self) -> ProviderResult<Option<BlockNumber>>;
}
