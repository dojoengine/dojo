use std::ops::Range;

use katana_primitives::block::{BlockHash, BlockHashOrNumber, BlockNumber, FinalityStatus};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::{TxHash, TxNumber, TxWithHash};

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TransactionProvider: Send + Sync {
    /// Returns a transaction given its hash.
    fn transaction_by_hash(&self, hash: TxHash) -> ProviderResult<Option<TxWithHash>>;

    /// Returns all the transactions for a given block.
    fn transactions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<TxWithHash>>>;

    /// Returns the transaction at the given block and its exact index in the block.
    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> ProviderResult<Option<TxWithHash>>;

    /// Returns the total number of transactions in a block.
    fn transaction_count_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<u64>>;

    /// Returns the block number and hash of a transaction.
    fn transaction_block_num_and_hash(
        &self,
        hash: TxHash,
    ) -> ProviderResult<Option<(BlockNumber, BlockHash)>>;

    /// Retrieves all the transactions at the given range.
    fn transaction_in_range(&self, _range: Range<TxNumber>) -> ProviderResult<Vec<TxWithHash>> {
        todo!()
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TransactionsProviderExt: TransactionProvider + Send + Sync {
    /// Retrieves the tx hashes for the given range of tx numbers.
    fn transaction_hashes_in_range(&self, range: Range<TxNumber>) -> ProviderResult<Vec<TxHash>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TransactionStatusProvider: Send + Sync {
    /// Retrieves the finality status of a transaction.
    fn transaction_status(&self, hash: TxHash) -> ProviderResult<Option<FinalityStatus>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ReceiptProvider: Send + Sync {
    /// Returns the transaction receipt given a transaction hash.
    fn receipt_by_hash(&self, hash: TxHash) -> ProviderResult<Option<Receipt>>;

    /// Returns all the receipts for a given block.
    fn receipts_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<Receipt>>>;
}
