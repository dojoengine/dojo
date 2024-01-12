use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::env::BlockEnv;

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockEnvProvider: Send + Sync {
    fn env_at(&self, block_id: BlockHashOrNumber) -> ProviderResult<BlockEnv>;
}
