use std::ops::Range;

use anyhow::Result;
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::transaction::{Receipt, Tx, TxHash, TxNumber};

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TransactionProvider: Send + Sync {
    /// Returns a transaction given its hash.
    fn transaction_by_hash(&self, hash: TxHash) -> Result<Option<Tx>>;

    /// Returns all the transactions for a given block.
    fn transactions_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Tx>>>;

    /// Returns the transaction at the given block and its exact index in the block.
    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> Result<Option<Tx>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TransactionsProviderExt: TransactionProvider + Send + Sync {
    /// Retrieves the tx hashes for the given range of tx numbers.
    fn transaction_hashes_by_range(&self, range: Range<TxNumber>) -> Result<Vec<TxHash>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ReceiptProvider: Send + Sync {
    /// Returns the transaction receipt given a transaction hash.
    fn receipt_by_hash(&self, hash: TxHash) -> Result<Option<Receipt>>;

    /// Returns all the receipts for a given block.
    fn receipts_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Receipt>>>;
}
