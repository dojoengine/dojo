use anyhow::Result;
use katana_primitives::block::BlockHashOrNumber;
use katana_provider::traits::block::{BlockHashProvider, BlockProvider, BlockStatusProvider};
use katana_rpc_types::block::{BlockWithTxHashes, BlockWithTxs};

/// A builder for building RPC block types.
pub struct BlockBuilder<P> {
    provider: P,
    block_id: BlockHashOrNumber,
}

impl<P> BlockBuilder<P> {
    pub fn new(block_id: BlockHashOrNumber, provider: P) -> Self {
        Self { provider, block_id }
    }
}

impl<P> BlockBuilder<P>
where
    P: BlockProvider + BlockHashProvider,
{
    pub fn build(self) -> Result<Option<BlockWithTxs>> {
        let Some(hash) = BlockHashProvider::block_hash_by_id(&self.provider, self.block_id)? else {
            return Ok(None);
        };

        let block = BlockProvider::block(&self.provider, self.block_id)?
            .expect("should exist if hash exists");
        let finality_status = BlockStatusProvider::block_status(&self.provider, self.block_id)?
            .expect("should exist if block exists");

        Ok(Some(BlockWithTxs::new(hash, block, finality_status)))
    }

    pub fn build_with_tx_hash(self) -> Result<Option<BlockWithTxHashes>> {
        let Some(hash) = BlockHashProvider::block_hash_by_id(&self.provider, self.block_id)? else {
            return Ok(None);
        };

        let block = BlockProvider::block_with_tx_hashes(&self.provider, self.block_id)?
            .expect("should exist if block exists");
        let finality_status = BlockStatusProvider::block_status(&self.provider, self.block_id)?
            .expect("should exist if block exists");

        Ok(Some(BlockWithTxHashes::new(hash, block, finality_status)))
    }
}
