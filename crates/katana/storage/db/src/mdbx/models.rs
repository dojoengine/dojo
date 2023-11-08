use serde::{Deserialize, Serialize};

/// The sequential number of all the transactions in the database.
pub type TxNumber = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredBlockBodyIndices {
    /// The offset in database of the first transaction in the block.
    ///
    /// `tx_offset` is a key of `Transactions` table.
    pub tx_offset: TxNumber,
    /// The total number of transactions in the block.
    pub tx_count: u64,
}
