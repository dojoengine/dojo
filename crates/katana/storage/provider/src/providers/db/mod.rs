use std::ops::{Range, RangeInclusive};

use anyhow::Result;
use katana_db::mdbx::DbEnv;
use katana_db::models::block::StoredBlockBodyIndices;
use katana_db::tables::{
    BlockBodyIndices, BlockHashes, BlockNumbers, BlockStatusses, Headers, Receipts, Transactions,
    TxBlocks, TxHashes, TxNumbers,
};
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockNumber, BlockWithTxHashes, FinalityStatus, Header,
};
use katana_primitives::receipt::Receipt;
use katana_primitives::state::StateUpdates;
use katana_primitives::transaction::{TxHash, TxNumber, TxWithHash};

use crate::traits::block::{
    BlockHashProvider, BlockNumberProvider, BlockProvider, BlockStatusProvider, HeaderProvider,
};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};

impl BlockNumberProvider for DbEnv {
    fn block_number_by_hash(&self, hash: BlockHash) -> Result<Option<BlockNumber>> {
        let db_tx = self.tx()?;
        let block_num = db_tx.get::<BlockNumbers>(hash)?;
        db_tx.commit()?;
        Ok(block_num)
    }

    fn latest_number(&self) -> Result<BlockNumber> {
        let db_tx = self.tx()?;
        let total_blocks = db_tx.entries::<BlockNumbers>()? as u64;
        db_tx.commit()?;
        Ok(if total_blocks == 0 { 0 } else { total_blocks - 1 })
    }
}

impl BlockHashProvider for DbEnv {
    fn latest_hash(&self) -> Result<BlockHash> {
        let db_tx = self.tx()?;
        let total_blocks = db_tx.entries::<BlockNumbers>()? as u64;
        let latest_block = if total_blocks == 0 { 0 } else { total_blocks - 1 };
        let latest_hash = db_tx.get::<BlockHashes>(latest_block)?.expect("should exist");
        db_tx.commit()?;
        Ok(latest_hash)
    }

    fn block_hash_by_num(&self, num: BlockNumber) -> Result<Option<BlockHash>> {
        let db_tx = self.tx()?;
        let block_hash = db_tx.get::<BlockHashes>(num)?;
        db_tx.commit()?;
        Ok(block_hash)
    }
}

impl HeaderProvider for DbEnv {
    fn header(&self, id: BlockHashOrNumber) -> Result<Option<Header>> {
        let db_tx = self.tx()?;

        let num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<BlockNumbers>(hash)?,
        };

        if let Some(num) = num {
            let header = db_tx.get::<Headers>(num)?.expect("should exist");
            db_tx.commit()?;
            Ok(Some(header))
        } else {
            Ok(None)
        }
    }
}

impl BlockProvider for DbEnv {
    fn block_body_indices(&self, id: BlockHashOrNumber) -> Result<Option<StoredBlockBodyIndices>> {
        let db_tx = self.tx()?;

        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<BlockNumbers>(hash)?,
        };

        if let Some(num) = block_num {
            let indices = db_tx.get::<BlockBodyIndices>(num)?;
            db_tx.commit()?;
            Ok(indices)
        } else {
            Ok(None)
        }
    }

    fn block(&self, id: BlockHashOrNumber) -> Result<Option<Block>> {
        let db_tx = self.tx()?;

        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<BlockNumbers>(hash)?,
        };

        let Some(block_num) = block_num else { return Ok(None) };

        if let Some(header) = db_tx.get::<Headers>(block_num)? {
            let body_indices = db_tx.get::<BlockBodyIndices>(block_num)?.expect("should exist");
            let body = self.transaction_in_range(Range::from(body_indices))?;
            let block = Block { header, body };

            db_tx.commit()?;

            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    fn block_with_tx_hashes(&self, id: BlockHashOrNumber) -> Result<Option<BlockWithTxHashes>> {
        let db_tx = self.tx()?;

        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<BlockNumbers>(hash)?,
        };

        let Some(block_num) = block_num else { return Ok(None) };

        if let Some(header) = db_tx.get::<Headers>(block_num)? {
            let body_indices = db_tx.get::<BlockBodyIndices>(block_num)?.expect("should exist");
            let body = self.transaction_hashes_in_range(Range::from(body_indices))?;
            let block = BlockWithTxHashes { header, body };

            db_tx.commit()?;

            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> Result<Vec<Block>> {
        let db_tx = self.tx()?;

        let total = range.end() - range.start() + 1;
        let mut blocks = Vec::with_capacity(total as usize);

        for num in range {
            if let Some(header) = db_tx.get::<Headers>(num)? {
                let body_indices = db_tx.get::<BlockBodyIndices>(num)?.expect("should exist");
                let body = self.transaction_in_range(Range::from(body_indices))?;
                blocks.push(Block { header, body })
            }
        }

        db_tx.commit()?;
        Ok(blocks)
    }
}

impl BlockStatusProvider for DbEnv {
    fn block_status(&self, id: BlockHashOrNumber) -> Result<Option<FinalityStatus>> {
        let db_tx = self.tx()?;

        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.block_number_by_hash(hash)?,
        };

        if let Some(block_num) = block_num {
            let status = db_tx.get::<BlockStatusses>(block_num)?.expect("should exist");
            db_tx.commit()?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }
}

impl TransactionProvider for DbEnv {
    fn transaction_by_hash(&self, hash: TxHash) -> Result<Option<TxWithHash>> {
        let db_tx = self.tx()?;

        if let Some(num) = db_tx.get::<TxNumbers>(hash)? {
            let transaction = db_tx.get::<Transactions>(num)?.expect("transaction should exist");
            let transaction = TxWithHash { hash, transaction };
            db_tx.commit()?;

            Ok(Some(transaction))
        } else {
            Ok(None)
        }
    }

    fn transactions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> Result<Option<Vec<TxWithHash>>> {
        if let Some(indices) = self.block_body_indices(block_id)? {
            Ok(Some(self.transaction_in_range(Range::from(indices))?))
        } else {
            Ok(None)
        }
    }

    fn transaction_in_range(&self, range: Range<TxNumber>) -> Result<Vec<TxWithHash>> {
        let db_tx = self.tx()?;

        let total = range.end - range.start;
        let mut transactions = Vec::with_capacity(total as usize);

        for i in range {
            if let Some(transaction) = db_tx.get::<Transactions>(i)? {
                let hash = db_tx.get::<TxHashes>(i)?.expect("should exist");
                transactions.push(TxWithHash { hash, transaction });
            };
        }

        db_tx.commit()?;
        Ok(transactions)
    }

    fn transaction_block_num_and_hash(
        &self,
        hash: TxHash,
    ) -> Result<Option<(BlockNumber, BlockHash)>> {
        let db_tx = self.tx()?;
        if let Some(num) = db_tx.get::<TxNumbers>(hash)? {
            let block_num = db_tx.get::<TxBlocks>(num)?.expect("should exist");
            let block_hash = db_tx.get::<BlockHashes>(block_num)?.expect("should exist");
            db_tx.commit()?;
            Ok(Some((block_num, block_hash)))
        } else {
            Ok(None)
        }
    }

    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> Result<Option<TxWithHash>> {
        let db_tx = self.tx()?;

        match self.block_body_indices(block_id)? {
            // make sure the requested idx is within the range of the block tx count
            Some(indices) if idx < indices.tx_count => {
                let num = indices.tx_offset + idx;
                let hash = db_tx.get::<TxHashes>(num)?.expect("should exist");
                let transaction = db_tx.get::<Transactions>(num)?.expect("should exist");
                let transaction = TxWithHash { hash, transaction };
                db_tx.commit()?;
                Ok(Some(transaction))
            }

            _ => Ok(None),
        }
    }

    fn transaction_count_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<u64>> {
        let db_tx = self.tx()?;
        if let Some(indices) = self.block_body_indices(block_id)? {
            db_tx.commit()?;
            Ok(Some(indices.tx_count))
        } else {
            Ok(None)
        }
    }
}

impl TransactionsProviderExt for DbEnv {
    fn transaction_hashes_in_range(&self, range: Range<TxNumber>) -> Result<Vec<TxHash>> {
        let db_tx = self.tx()?;

        let total = range.end - range.start;
        let mut hashes = Vec::with_capacity(total as usize);

        for i in range {
            if let Some(hash) = db_tx.get::<TxHashes>(i)? {
                hashes.push(hash);
            }
        }

        db_tx.commit()?;
        Ok(hashes)
    }
}

impl TransactionStatusProvider for DbEnv {
    fn transaction_status(&self, hash: TxHash) -> Result<Option<FinalityStatus>> {
        let db_tx = self.tx()?;
        if let Some(num) = db_tx.get::<TxNumbers>(hash)? {
            let status = db_tx.get::<BlockStatusses>(num)?.expect("should exist");
            db_tx.commit()?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }
}

impl ReceiptProvider for DbEnv {
    fn receipt_by_hash(&self, hash: TxHash) -> Result<Option<Receipt>> {
        let db_tx = self.tx()?;
        if let Some(num) = db_tx.get::<TxNumbers>(hash)? {
            let receipt = db_tx.get::<katana_db::tables::Receipts>(num)?.expect("should exist");
            db_tx.commit()?;
            Ok(Some(receipt))
        } else {
            Ok(None)
        }
    }

    fn receipts_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Receipt>>> {
        if let Some(indices) = self.block_body_indices(block_id)? {
            let db_tx = self.tx()?;
            let mut receipts = Vec::with_capacity(indices.tx_count as usize);

            let range = indices.tx_offset..indices.tx_offset + indices.tx_count;
            for i in range {
                if let Some(receipt) = db_tx.get::<Receipts>(i)? {
                    receipts.push(receipt);
                }
            }

            db_tx.commit()?;
            Ok(Some(receipts))
        } else {
            Ok(None)
        }
    }
}

impl StateUpdateProvider for DbEnv {
    fn state_update(&self, _block_id: BlockHashOrNumber) -> Result<Option<StateUpdates>> {
        todo!()
    }
}
