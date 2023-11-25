use std::ops::RangeInclusive;

use anyhow::Result;
use katana_primitives::block::{Block, BlockHash, BlockHashOrNumber, BlockNumber, Header};

use super::transaction::TransactionProvider;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockHashProvider: Send + Sync {
    /// Retrieves the latest block hash.
    ///
    /// There should always be at least one block (genesis) in the chain.
    fn latest_hash(&self) -> Result<BlockHash>;

    /// Retrieves the block hash given its id.
    fn block_hash_by_num(&self, num: BlockNumber) -> Result<Option<BlockHash>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockNumberProvider: Send + Sync {
    /// Retrieves the latest block number.
    ///
    /// There should always be at least one block (genesis) in the chain.
    fn latest_number(&self) -> Result<BlockNumber>;

    /// Retrieves the block number given its id.
    fn block_number_by_hash(&self, hash: BlockHash) -> Result<Option<BlockNumber>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait HeaderProvider: Send + Sync {
    /// Retrieves the latest header by its block id.
    fn header(&self, id: BlockHashOrNumber) -> Result<Option<Header>>;

    fn header_by_hash(&self, hash: BlockHash) -> Result<Option<Header>> {
        self.header(hash.into())
    }

    fn header_by_number(&self, number: BlockNumber) -> Result<Option<Header>> {
        self.header(number.into())
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockProvider:
    BlockHashProvider + BlockNumberProvider + HeaderProvider + TransactionProvider + Send + Sync
{
    /// Returns a block by its id.
    fn block(&self, id: BlockHashOrNumber) -> Result<Option<Block>>;

    /// Returns all available blocks in the given range.
    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> Result<Vec<Block>>;

    /// Returns the block based on its hash.
    fn block_by_hash(&self, hash: BlockHash) -> Result<Option<Block>> {
        self.block(hash.into())
    }

    /// Returns the block based on its number.
    fn block_by_number(&self, number: BlockNumber) -> Result<Option<Block>> {
        self.block(number.into())
    }
}

pub trait BlockExecutionWriter: Send + Sync {
    /// Store an executed block along with its output to the storage.
    fn store_block(&mut self, block: Block) -> Result<()>;
}
