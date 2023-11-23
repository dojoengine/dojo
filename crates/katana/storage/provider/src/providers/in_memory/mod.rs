pub mod state;

use std::collections::HashMap;
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

use self::state::{HistoricalStates, InMemoryState, LatestStateProvider, SnapshotStateProvider};
use crate::traits::block::{BlockHashProvider, BlockNumberProvider, BlockProvider, HeaderProvider};
use crate::traits::contract::ContractProvider;
use crate::traits::state::{StateFactoryProvider, StateProvider};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{ReceiptProvider, TransactionProvider, TransactionsProviderExt};

#[derive(Default)]
pub struct InMemoryProvider {
    pub block_headers: HashMap<BlockNumber, Header>,
    pub block_hashes: HashMap<BlockNumber, BlockHash>,
    pub block_numbers: HashMap<BlockHash, BlockNumber>,
    pub block_body_indices: HashMap<BlockNumber, StoredBlockBodyIndices>,

    pub latest_block_number: BlockNumber,
    pub latest_block_hash: BlockHash,

    pub state_update: HashMap<BlockNumber, StateUpdate>,

    pub transactions: Vec<Transaction>,
    pub transaction_numbers: HashMap<TxHash, TxNumber>,
    pub transaction_hashes: HashMap<TxNumber, TxHash>,
    pub receipts: Vec<Receipt>,

    pub state: Arc<InMemoryState>,

    pub historical_states: RwLock<HistoricalStates>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BlockHashProvider for InMemoryProvider {
    fn latest_hash(&self) -> Result<BlockHash> {
        Ok(self.latest_block_hash)
    }

    fn block_hash_by_num(&self, num: BlockNumber) -> Result<Option<BlockHash>> {
        Ok(self.block_hashes.get(&num).cloned())
    }
}

impl BlockNumberProvider for InMemoryProvider {
    fn latest_number(&self) -> Result<BlockNumber> {
        Ok(self.latest_block_number)
    }

    fn block_number_by_hash(&self, hash: BlockHash) -> Result<Option<BlockNumber>> {
        Ok(self.block_numbers.get(&hash).cloned())
    }
}

impl HeaderProvider for InMemoryProvider {
    fn header(&self, id: katana_primitives::block::BlockHashOrNumber) -> Result<Option<Header>> {
        match id {
            katana_primitives::block::BlockHashOrNumber::Num(num) => {
                Ok(self.block_headers.get(&num).cloned())
            }

            katana_primitives::block::BlockHashOrNumber::Hash(hash) => {
                let header @ Some(_) = self
                    .block_numbers
                    .get(&hash)
                    .and_then(|num| self.block_headers.get(num).cloned())
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
            BlockHashOrNumber::Hash(hash) => self.block_numbers.get(&hash).cloned(),
        };

        let Some(header) = block_num.and_then(|num| self.block_headers.get(&num).cloned()) else {
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
            .transaction_numbers
            .get(&hash)
            .and_then(|num| self.transactions.get(*num as usize).cloned()))
    }

    fn transactions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> Result<Option<Vec<Transaction>>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.block_body_indices.get(&num))
        else {
            return Ok(None);
        };

        let offset = *tx_offset as usize;
        let count = *tx_count as usize;

        Ok(Some(self.transactions[offset..offset + count].to_vec()))
    }

    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> Result<Option<Transaction>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.block_body_indices.get(&num))
        else {
            return Ok(None);
        };

        let offset = *tx_offset as usize;

        if idx >= *tx_count {
            return Ok(None);
        }

        Ok(Some(self.transactions[offset + idx as usize].clone()))
    }
}

impl TransactionsProviderExt for InMemoryProvider {
    fn transaction_hashes_by_range(&self, range: std::ops::Range<TxNumber>) -> Result<Vec<TxHash>> {
        let mut hashes = Vec::new();
        for num in range {
            if let Some(hash) = self.transaction_hashes.get(&num).cloned() {
                hashes.push(hash);
            }
        }
        Ok(hashes)
    }
}

impl ReceiptProvider for InMemoryProvider {
    fn receipt_by_hash(&self, hash: TxHash) -> Result<Option<Receipt>> {
        let receipt = self
            .transaction_numbers
            .get(&hash)
            .and_then(|num| self.receipts.get(*num as usize).cloned());
        Ok(receipt)
    }

    fn receipts_by_block(&self, block_id: BlockHashOrNumber) -> Result<Option<Vec<Receipt>>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.block_body_indices.get(&num))
        else {
            return Ok(None);
        };

        let offset = *tx_offset as usize;
        let count = *tx_count as usize;

        Ok(Some(self.receipts[offset..offset + count].to_vec()))
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
            BlockHashOrNumber::Hash(hash) => self.block_numbers.get(&hash).cloned(),
        };

        let state_update = block_num.and_then(|num| self.state_update.get(&num).cloned());
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

        let provider @ Some(_) =
            block_num.and_then(|num| {
                self.historical_states.read().get(&num).cloned().map(|provider| {
                    Box::new(SnapshotStateProvider(provider)) as Box<dyn StateProvider>
                })
            })
        else {
            return Ok(None);
        };

        Ok(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryProvider;

    pub(super) fn create_mock_provider() -> InMemoryProvider {
        InMemoryProvider { ..Default::default() }
    }
}
