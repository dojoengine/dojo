use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::env::BlockEnv;

use crate::ProviderResult;

/// A provider that provides block environment values including Starknet execution environment
/// values.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockEnvProvider: Send + Sync {
    /// Returns the block environment values at the given block id.
    fn block_env_at(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<BlockEnv>>;
}
