use std::ops::Range;

use anyhow::Result;
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::transaction::{Receipt, Transaction, TxHash, TxNumber};

pub trait TransactionProvider {
    /// Returns a transaction given its hash.
    fn transaction_by_hash(&self, hash: TxHash) -> Result<Option<Transaction>>;

    /// Returns all the transactions for a given block.
    fn transactions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> Result<Option<Vec<Transaction>>>;

    /// Returns the transaction at the given block and its exact index in the block.
    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> Result<Option<Transaction>>;
}

pub trait TransactionsProviderExt {
    /// Retrieves the tx hashes for the given range of tx numbers.
    fn transaction_hashes_by_range(&self, range: Range<TxNumber>) -> Result<Vec<TxHash>>;
}

pub trait ReceiptProvider {
    /// Returns the transaction receipt given a transaction hash.
    fn receipt_by_hash(&self, hash: TxHash) -> Result<Option<Receipt>>;

    /// Returns all the receipts for a given block.
    fn receipts_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Receipt>>>;
}
