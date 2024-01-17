use std::ops::RangeInclusive;

use katana_db::models::block::StoredBlockBodyIndices;
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockIdOrTag, BlockNumber, BlockTag, BlockWithTxHashes,
    FinalityStatus, Header, SealedBlockWithStatus,
};
use katana_primitives::receipt::Receipt;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;

use super::transaction::{TransactionProvider, TransactionsProviderExt};
use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockIdReader: BlockNumberProvider + Send + Sync {
    /// Converts the block tag into its block number.
    fn convert_block_id(&self, id: BlockIdOrTag) -> ProviderResult<Option<BlockNumber>> {
        match id {
            BlockIdOrTag::Number(number) => Ok(Some(number)),
            BlockIdOrTag::Hash(hash) => BlockNumberProvider::block_number_by_hash(self, hash),

            BlockIdOrTag::Tag(BlockTag::Latest) => {
                BlockNumberProvider::latest_number(&self).map(Some)
            }

            BlockIdOrTag::Tag(BlockTag::Pending) => {
                if let Some((num, _)) = Self::pending_block_id(self)? {
                    Ok(Some(num))
                } else {
                    // returns latest number for now
                    BlockNumberProvider::latest_number(&self).map(Some)
                }
            }
        }
    }

    // TODO: integrate the pending block with the provider
    /// Retrieves the pending block number and hash.
    fn pending_block_id(&self) -> ProviderResult<Option<(BlockNumber, BlockHash)>> {
        Ok(None) // Returns `None` for now
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockHashProvider: Send + Sync {
    /// Retrieves the latest block hash.
    ///
    /// There should always be at least one block (genesis) in the chain.
    fn latest_hash(&self) -> ProviderResult<BlockHash>;

    /// Retrieves the block hash given its id.
    fn block_hash_by_num(&self, num: BlockNumber) -> ProviderResult<Option<BlockHash>>;

    /// Retrieves the block hash given its id.
    fn block_hash_by_id(&self, id: BlockHashOrNumber) -> ProviderResult<Option<BlockHash>> {
        match id {
            BlockHashOrNumber::Hash(hash) => Ok(Some(hash)),
            BlockHashOrNumber::Num(number) => self.block_hash_by_num(number),
        }
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockNumberProvider: Send + Sync {
    /// Retrieves the latest block number.
    ///
    /// There should always be at least one block (genesis) in the chain.
    fn latest_number(&self) -> ProviderResult<BlockNumber>;

    /// Retrieves the block number given its id.
    fn block_number_by_hash(&self, hash: BlockHash) -> ProviderResult<Option<BlockNumber>>;

    /// Retrieves the block number given its id.
    fn block_number_by_id(&self, id: BlockHashOrNumber) -> ProviderResult<Option<BlockNumber>> {
        match id {
            BlockHashOrNumber::Num(number) => Ok(Some(number)),
            BlockHashOrNumber::Hash(hash) => self.block_number_by_hash(hash),
        }
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait HeaderProvider: Send + Sync {
    /// Retrieves the latest header by its block id.
    fn header(&self, id: BlockHashOrNumber) -> ProviderResult<Option<Header>>;

    fn header_by_hash(&self, hash: BlockHash) -> ProviderResult<Option<Header>> {
        self.header(hash.into())
    }

    fn header_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Header>> {
        self.header(number.into())
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockStatusProvider: Send + Sync {
    /// Retrieves the finality status of a block.
    fn block_status(&self, id: BlockHashOrNumber) -> ProviderResult<Option<FinalityStatus>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockProvider:
    BlockHashProvider
    + BlockNumberProvider
    + HeaderProvider
    + BlockStatusProvider
    + TransactionProvider
    + TransactionsProviderExt
    + Send
    + Sync
{
    /// Returns a block by its id.
    fn block(&self, id: BlockHashOrNumber) -> ProviderResult<Option<Block>>;

    /// Returns a block with only the transaction hashes.
    fn block_with_tx_hashes(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BlockWithTxHashes>>;

    /// Returns all available blocks in the given range.
    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> ProviderResult<Vec<Block>>;

    /// Returns the block body indices of a block.
    fn block_body_indices(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<StoredBlockBodyIndices>>;

    /// Returns the block based on its hash.
    fn block_by_hash(&self, hash: BlockHash) -> ProviderResult<Option<Block>> {
        self.block(hash.into())
    }

    /// Returns the block based on its number.
    fn block_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Block>> {
        self.block(number.into())
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BlockWriter: Send + Sync {
    /// Store an executed block along with its execution output to the storage.
    fn insert_block_with_states_and_receipts(
        &self,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithDeclaredClasses,
        receipts: Vec<Receipt>,
    ) -> ProviderResult<()>;
}
