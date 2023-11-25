pub mod cache;
pub mod state;

use std::ops::RangeInclusive;
use std::sync::Arc;

use anyhow::Result;
use katana_db::models::block::StoredBlockBodyIndices;
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockNumber, Header, StateUpdate,
};
use katana_primitives::contract::{ContractAddress, GenericContractInfo};
use katana_primitives::transaction::{Receipt, Transaction, TxHash, TxNumber};
use parking_lot::RwLock;

use self::cache::CacheDb;
use self::state::{HistoricalStates, InMemoryStateDb, LatestStateProvider};
use crate::traits::block::{BlockHashProvider, BlockNumberProvider, BlockProvider, HeaderProvider};
use crate::traits::contract::ContractProvider;
use crate::traits::state::{StateFactoryProvider, StateProvider};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{ReceiptProvider, TransactionProvider, TransactionsProviderExt};

pub struct InMemoryProvider {
    storage: CacheDb<()>,
    state: Arc<InMemoryStateDb>,
    historical_states: RwLock<HistoricalStates>,
}

impl BlockHashProvider for InMemoryProvider {
    fn latest_hash(&self) -> Result<BlockHash> {
        Ok(self.storage.latest_block_hash)
    }

    fn block_hash_by_num(&self, num: BlockNumber) -> Result<Option<BlockHash>> {
        Ok(self.storage.block_hashes.get(&num).cloned())
    }
}

impl BlockNumberProvider for InMemoryProvider {
    fn latest_number(&self) -> Result<BlockNumber> {
        Ok(self.storage.latest_block_number)
    }

    fn block_number_by_hash(&self, hash: BlockHash) -> Result<Option<BlockNumber>> {
        Ok(self.storage.block_numbers.get(&hash).cloned())
    }
}

impl HeaderProvider for InMemoryProvider {
    fn header(&self, id: katana_primitives::block::BlockHashOrNumber) -> Result<Option<Header>> {
        match id {
            katana_primitives::block::BlockHashOrNumber::Num(num) => {
                Ok(self.storage.block_headers.get(&num).cloned())
            }

            katana_primitives::block::BlockHashOrNumber::Hash(hash) => {
                let header @ Some(_) = self
                    .storage
                    .block_numbers
                    .get(&hash)
                    .and_then(|num| self.storage.block_headers.get(num).cloned())
                else {
                    return Ok(None);
                };
                Ok(header)
            }
        }
    }
}

impl BlockProvider for InMemoryProvider {
    fn block(&self, id: BlockHashOrNumber) -> Result<Option<Block>> {
        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.block_numbers.get(&hash).cloned(),
        };

        let Some(header) = block_num.and_then(|num| self.storage.block_headers.get(&num).cloned())
        else {
            return Ok(None);
        };

        let body = self.transactions_by_block(id)?.unwrap_or_default();

        Ok(Some(Block { header, body }))
    }

    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();
        for num in range {
            if let Some(block) = self.block(BlockHashOrNumber::Num(num))? {
                blocks.push(block);
            }
        }
        Ok(blocks)
    }
}

impl TransactionProvider for InMemoryProvider {
    fn transaction_by_hash(&self, hash: TxHash) -> Result<Option<Transaction>> {
        Ok(self
            .storage
            .transaction_numbers
            .get(&hash)
            .and_then(|num| self.storage.transactions.get(*num as usize).cloned()))
    }

    fn transactions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> Result<Option<Vec<Transaction>>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.storage.block_body_indices.get(&num))
        else {
            return Ok(None);
        };

        let offset = *tx_offset as usize;
        let count = *tx_count as usize;

        Ok(Some(self.storage.transactions[offset..offset + count].to_vec()))
    }

    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> Result<Option<Transaction>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.storage.block_body_indices.get(&num))
        else {
            return Ok(None);
        };

        let offset = *tx_offset as usize;

        if idx >= *tx_count {
            return Ok(None);
        }

        Ok(Some(self.storage.transactions[offset + idx as usize].clone()))
    }
}

impl TransactionsProviderExt for InMemoryProvider {
    fn transaction_hashes_by_range(&self, range: std::ops::Range<TxNumber>) -> Result<Vec<TxHash>> {
        let mut hashes = Vec::new();
        for num in range {
            if let Some(hash) = self.storage.transaction_hashes.get(&num).cloned() {
                hashes.push(hash);
            }
        }
        Ok(hashes)
    }
}

impl ReceiptProvider for InMemoryProvider {
    fn receipt_by_hash(&self, hash: TxHash) -> Result<Option<Receipt>> {
        let receipt = self
            .storage
            .transaction_numbers
            .get(&hash)
            .and_then(|num| self.storage.receipts.get(*num as usize).cloned());
        Ok(receipt)
    }

    fn receipts_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Receipt>>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.storage.block_body_indices.get(&num))
        else {
            return Ok(None);
        };

        let offset = *tx_offset as usize;
        let count = *tx_count as usize;

        Ok(Some(self.storage.receipts[offset..offset + count].to_vec()))
    }
}

impl ContractProvider for InMemoryProvider {
    fn contract(&self, address: ContractAddress) -> Result<Option<GenericContractInfo>> {
        let contract = self.state.contract_state.read().get(&address).cloned();
        Ok(contract)
    }
}

impl StateUpdateProvider for InMemoryProvider {
    fn state_update(&self, block_id: BlockHashOrNumber) -> Result<Option<StateUpdate>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.block_numbers.get(&hash).cloned(),
        };

        let state_update = block_num.and_then(|num| self.storage.state_update.get(&num).cloned());
        Ok(state_update)
    }
}

impl StateFactoryProvider for InMemoryProvider {
    fn latest(&self) -> Result<Box<dyn StateProvider>> {
        Ok(Box::new(LatestStateProvider(Arc::clone(&self.state))))
    }

    fn historical(&self, block_id: BlockHashOrNumber) -> Result<Option<Box<dyn StateProvider>>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.block_number_by_hash(hash)?,
        };

        let provider @ Some(_) = block_num.and_then(|num| {
            self.historical_states
                .read()
                .get(&num)
                .cloned()
                .map(|provider| Box::new(provider) as Box<dyn StateProvider>)
        }) else {
            return Ok(None);
        };

        Ok(provider)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::cache::{CacheDb, CacheStateDb};
    use super::InMemoryProvider;

    pub(super) fn create_mock_provider() -> InMemoryProvider {
        let storage = CacheDb::new(());
        let state = Arc::new(CacheStateDb::new(()));
        let historical_states = Default::default();
        InMemoryProvider { storage, state, historical_states }
    }
}
