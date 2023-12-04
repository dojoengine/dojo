use anyhow::Result;
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::env::BlockEnv;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockEnvProvider: Send + Sync {
    fn env_at(&self, block_id: BlockHashOrNumber) -> Result<BlockEnv>;
}
