use std::ops::Range;

use katana_primitives::transaction::TxNumber;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(::arbitrary::Arbitrary))]
pub struct StoredBlockBodyIndices {
    /// The offset in database of the first transaction in the block.
    ///
    /// `tx_offset` is a key of `Transactions` table.
    pub tx_offset: TxNumber,
    /// The total number of transactions in the block.
    pub tx_count: u64,
}

impl From<StoredBlockBodyIndices> for Range<u64> {
    fn from(value: StoredBlockBodyIndices) -> Self {
        let start = value.tx_offset;
        let end = value.tx_offset + value.tx_count;
        std::ops::Range { start, end }
    }
}
