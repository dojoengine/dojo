pub mod backend;
pub mod state;

use std::ops::{Range, RangeInclusive};
use std::sync::Arc;

use katana_db::models::block::StoredBlockBodyIndices;
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockNumber, BlockWithTxHashes, FinalityStatus, Header,
    SealedBlockWithStatus,
};
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::ContractAddress;
use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::Receipt;
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::transaction::{Tx, TxHash, TxNumber, TxWithHash};
use parking_lot::RwLock;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;

use self::backend::{ForkedBackend, ForkedBackendError, SharedStateProvider};
use self::state::ForkedStateDb;
use super::in_memory::cache::{CacheDb, CacheStateDb};
use super::in_memory::state::HistoricalStates;
use crate::traits::block::{
    BlockHashProvider, BlockNumberProvider, BlockProvider, BlockStatusProvider, BlockWriter,
    HeaderProvider,
};
use crate::traits::contract::ContractClassWriter;
use crate::traits::env::BlockEnvProvider;
use crate::traits::state::{StateFactoryProvider, StateProvider, StateRootProvider, StateWriter};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};
use crate::ProviderResult;

pub struct ForkedProvider {
    // TODO: insert `ForkedBackend` into `CacheDb`
    storage: RwLock<CacheDb<()>>,
    state: Arc<ForkedStateDb>,
    historical_states: RwLock<HistoricalStates>,
}

impl ForkedProvider {
    pub fn new(
        provider: Arc<JsonRpcClient<HttpTransport>>,
        block_id: BlockHashOrNumber,
    ) -> Result<Self, ForkedBackendError> {
        let backend = ForkedBackend::new_with_backend_thread(provider, block_id)?;
        let shared_provider = SharedStateProvider::new_with_backend(backend);

        let storage = RwLock::new(CacheDb::new(()));
        let state = Arc::new(CacheStateDb::new(shared_provider));
        let historical_states = RwLock::new(HistoricalStates::default());

        Ok(Self { storage, state, historical_states })
    }
}

impl BlockHashProvider for ForkedProvider {
    fn latest_hash(&self) -> ProviderResult<BlockHash> {
        Ok(self.storage.read().latest_block_hash)
    }

    fn block_hash_by_num(&self, num: BlockNumber) -> ProviderResult<Option<BlockHash>> {
        Ok(self.storage.read().block_hashes.get(&num).cloned())
    }
}

impl BlockNumberProvider for ForkedProvider {
    fn latest_number(&self) -> ProviderResult<BlockNumber> {
        Ok(self.storage.read().latest_block_number)
    }

    fn block_number_by_hash(&self, hash: BlockHash) -> ProviderResult<Option<BlockNumber>> {
        Ok(self.storage.read().block_numbers.get(&hash).cloned())
    }
}

impl HeaderProvider for ForkedProvider {
    fn header(
        &self,
        id: katana_primitives::block::BlockHashOrNumber,
    ) -> ProviderResult<Option<Header>> {
        match id {
            katana_primitives::block::BlockHashOrNumber::Num(num) => {
                Ok(self.storage.read().block_headers.get(&num).cloned())
            }

            katana_primitives::block::BlockHashOrNumber::Hash(hash) => {
                let header @ Some(_) = self
                    .storage
                    .read()
                    .block_numbers
                    .get(&hash)
                    .and_then(|num| self.storage.read().block_headers.get(num).cloned())
                else {
                    return Ok(None);
                };
                Ok(header)
            }
        }
    }
}

impl BlockStatusProvider for ForkedProvider {
    fn block_status(&self, id: BlockHashOrNumber) -> ProviderResult<Option<FinalityStatus>> {
        let num = match id {
            BlockHashOrNumber::Num(num) => num,
            BlockHashOrNumber::Hash(hash) => {
                match self.storage.read().block_numbers.get(&hash).copied() {
                    Some(num) => num,
                    None => return Ok(None),
                }
            }
        };
        Ok(self.storage.read().block_statusses.get(&num).cloned())
    }
}

impl BlockProvider for ForkedProvider {
    fn block(&self, id: BlockHashOrNumber) -> ProviderResult<Option<Block>> {
        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.read().block_numbers.get(&hash).cloned(),
        };

        let Some(header) =
            block_num.and_then(|num| self.storage.read().block_headers.get(&num).cloned())
        else {
            return Ok(None);
        };

        let body = self.transactions_by_block(id)?.unwrap_or_default();

        Ok(Some(Block { header, body }))
    }

    fn block_with_tx_hashes(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BlockWithTxHashes>> {
        let Some(header) = self.header(id)? else {
            return Ok(None);
        };

        let tx_range = self.block_body_indices(id)?.expect("should exist");
        let tx_hashes = self.transaction_hashes_in_range(tx_range.into())?;

        Ok(Some(katana_primitives::block::BlockWithTxHashes { header, body: tx_hashes }))
    }

    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> ProviderResult<Vec<Block>> {
        let mut blocks = Vec::new();
        for num in range {
            if let Some(block) = self.block(BlockHashOrNumber::Num(num))? {
                blocks.push(block);
            }
        }
        Ok(blocks)
    }

    fn block_body_indices(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<StoredBlockBodyIndices>> {
        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.read().block_numbers.get(&hash).cloned(),
        };

        let Some(indices) =
            block_num.and_then(|num| self.storage.read().block_body_indices.get(&num).cloned())
        else {
            return Ok(None);
        };

        Ok(Some(indices))
    }
}

impl TransactionProvider for ForkedProvider {
    fn transaction_by_hash(&self, hash: TxHash) -> ProviderResult<Option<TxWithHash>> {
        let tx = self.storage.read().transaction_numbers.get(&hash).and_then(|num| {
            let transaction = self.storage.read().transactions.get(*num as usize).cloned()?;
            let hash = self.storage.read().transaction_hashes.get(num).copied()?;
            Some(TxWithHash { hash, transaction })
        });
        Ok(tx)
    }

    fn transactions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<TxWithHash>>> {
        if let Some(indices) = self.block_body_indices(block_id)? {
            Ok(Some(self.transaction_in_range(Range::from(indices))?))
        } else {
            Ok(None)
        }
    }

    fn transaction_by_block_and_idx(
        &self,
        block_id: BlockHashOrNumber,
        idx: u64,
    ) -> ProviderResult<Option<TxWithHash>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.read().block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.storage.read().block_body_indices.get(&num).cloned())
        else {
            return Ok(None);
        };

        let offset = tx_offset as usize;

        if idx >= tx_count {
            return Ok(None);
        }

        let id = offset + idx as usize;

        let tx = self.storage.read().transactions.get(id).cloned().and_then(|tx| {
            let hash = self.storage.read().transaction_hashes.get(&(id as u64)).copied()?;
            Some(TxWithHash { hash, transaction: tx })
        });

        Ok(tx)
    }

    fn transaction_count_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<u64>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.read().block_numbers.get(&hash).cloned(),
        };

        let Some(tx_count) = block_num
            .and_then(|n| self.storage.read().block_body_indices.get(&n).map(|b| b.tx_count))
        else {
            return Ok(None);
        };

        Ok(Some(tx_count))
    }

    fn transaction_block_num_and_hash(
        &self,
        hash: TxHash,
    ) -> ProviderResult<Option<(BlockNumber, BlockHash)>> {
        let storage_read = self.storage.read();

        let Some(number) = storage_read.transaction_numbers.get(&hash) else { return Ok(None) };
        let block_num = storage_read.transaction_block.get(number).expect("block num should exist");
        let block_hash = storage_read.block_hashes.get(block_num).expect("block hash should exist");

        Ok(Some((*block_num, *block_hash)))
    }

    fn transaction_in_range(&self, range: Range<TxNumber>) -> ProviderResult<Vec<TxWithHash>> {
        let start = range.start as usize;
        let total = range.end as usize - start;

        let txs = self
            .storage
            .read()
            .transactions
            .iter()
            .enumerate()
            .skip(start)
            .take(total)
            .map(|(n, tx)| {
                let hash =
                    self.storage.read().transaction_hashes.get(&(n as u64)).cloned().unwrap();
                TxWithHash { hash, transaction: tx.clone() }
            })
            .collect::<Vec<TxWithHash>>();

        Ok(txs)
    }
}

impl TransactionsProviderExt for ForkedProvider {
    fn transaction_hashes_in_range(
        &self,
        range: std::ops::Range<TxNumber>,
    ) -> ProviderResult<Vec<TxHash>> {
        let mut hashes = Vec::new();
        for num in range {
            if let Some(hash) = self.storage.read().transaction_hashes.get(&num).cloned() {
                hashes.push(hash);
            }
        }
        Ok(hashes)
    }
}

impl TransactionStatusProvider for ForkedProvider {
    fn transaction_status(&self, hash: TxHash) -> ProviderResult<Option<FinalityStatus>> {
        let tx_block = self
            .storage
            .read()
            .transaction_numbers
            .get(&hash)
            .and_then(|n| self.storage.read().transaction_block.get(n).copied());

        if let Some(num) = tx_block {
            let status = self.block_status(num.into())?;
            Ok(status)
        } else {
            Ok(None)
        }
    }
}

impl ReceiptProvider for ForkedProvider {
    fn receipt_by_hash(&self, hash: TxHash) -> ProviderResult<Option<Receipt>> {
        let receipt = self
            .storage
            .read()
            .transaction_numbers
            .get(&hash)
            .and_then(|num| self.storage.read().receipts.get(*num as usize).cloned());
        Ok(receipt)
    }

    fn receipts_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<Receipt>>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.read().block_numbers.get(&hash).cloned(),
        };

        let Some(StoredBlockBodyIndices { tx_offset, tx_count }) =
            block_num.and_then(|num| self.storage.read().block_body_indices.get(&num).cloned())
        else {
            return Ok(None);
        };

        let offset = tx_offset as usize;
        let count = tx_count as usize;

        Ok(Some(self.storage.read().receipts[offset..offset + count].to_vec()))
    }
}

impl StateRootProvider for ForkedProvider {
    fn state_root(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<katana_primitives::FieldElement>> {
        let state_root = self.block_number_by_id(block_id)?.and_then(|num| {
            self.storage.read().block_headers.get(&num).map(|header| header.state_root)
        });
        Ok(state_root)
    }
}

impl StateUpdateProvider for ForkedProvider {
    fn state_update(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<StateUpdates>> {
        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.storage.read().block_numbers.get(&hash).cloned(),
        };

        let state_update =
            block_num.and_then(|num| self.storage.read().state_update.get(&num).cloned());
        Ok(state_update)
    }
}

impl StateFactoryProvider for ForkedProvider {
    fn latest(&self) -> ProviderResult<Box<dyn StateProvider>> {
        Ok(Box::new(self::state::LatestStateProvider(Arc::clone(&self.state))))
    }

    fn historical(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Box<dyn StateProvider>>> {
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

impl BlockWriter for ForkedProvider {
    fn insert_block_with_states_and_receipts(
        &self,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithDeclaredClasses,
        receipts: Vec<Receipt>,
    ) -> ProviderResult<()> {
        let mut storage = self.storage.write();

        let block_hash = block.block.header.hash;
        let block_number = block.block.header.header.number;

        let block_header = block.block.header.header;
        let txs = block.block.body;

        // create block body indices
        let tx_count = txs.len() as u64;
        let tx_offset = storage.transactions.len() as u64;
        let block_body_indices = StoredBlockBodyIndices { tx_offset, tx_count };

        let (txs_id, txs): (Vec<(TxNumber, TxHash)>, Vec<Tx>) = txs
            .into_iter()
            .enumerate()
            .map(|(num, tx)| ((num as u64 + tx_offset, tx.hash), tx.transaction))
            .unzip();

        let txs_num = txs_id.clone().into_iter().map(|(num, hash)| (hash, num));
        let txs_block = txs_id.clone().into_iter().map(|(num, _)| (num, block_number));

        storage.latest_block_hash = block_hash;
        storage.latest_block_number = block_number;

        storage.block_numbers.insert(block_hash, block_number);
        storage.block_hashes.insert(block_number, block_hash);
        storage.block_headers.insert(block_number, block_header);
        storage.block_statusses.insert(block_number, block.status);
        storage.block_body_indices.insert(block_number, block_body_indices);

        storage.transactions.extend(txs);
        storage.transaction_hashes.extend(txs_id);
        storage.transaction_numbers.extend(txs_num);
        storage.transaction_block.extend(txs_block);
        storage.receipts.extend(receipts);

        storage.state_update.insert(block_number, states.state_updates.clone());

        self.state.insert_updates(states);

        let snapshot = self.state.create_snapshot();
        self.historical_states.write().insert(block_number, Box::new(snapshot));

        Ok(())
    }
}

impl ContractClassWriter for ForkedProvider {
    fn set_class(&self, hash: ClassHash, class: CompiledClass) -> ProviderResult<()> {
        self.state.shared_contract_classes.compiled_classes.write().insert(hash, class);
        Ok(())
    }

    fn set_sierra_class(
        &self,
        hash: ClassHash,
        sierra: FlattenedSierraClass,
    ) -> ProviderResult<()> {
        self.state.shared_contract_classes.sierra_classes.write().insert(hash, sierra);
        Ok(())
    }

    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> ProviderResult<()> {
        self.state.compiled_class_hashes.write().insert(hash, compiled_hash);
        Ok(())
    }
}

impl StateWriter for ForkedProvider {
    fn set_storage(
        &self,
        address: ContractAddress,
        storage_key: katana_primitives::contract::StorageKey,
        storage_value: katana_primitives::contract::StorageValue,
    ) -> ProviderResult<()> {
        self.state.storage.write().entry(address).or_default().insert(storage_key, storage_value);
        Ok(())
    }

    fn set_class_hash_of_contract(
        &self,
        address: ContractAddress,
        class_hash: ClassHash,
    ) -> ProviderResult<()> {
        self.state.contract_state.write().entry(address).or_default().class_hash = class_hash;
        Ok(())
    }

    fn set_nonce(
        &self,
        address: ContractAddress,
        nonce: katana_primitives::contract::Nonce,
    ) -> ProviderResult<()> {
        self.state.contract_state.write().entry(address).or_default().nonce = nonce;
        Ok(())
    }
}

impl BlockEnvProvider for ForkedProvider {
    fn block_env_at(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<BlockEnv>> {
        Ok(self.header(block_id)?.map(|header| BlockEnv {
            number: header.number,
            timestamp: header.timestamp,
            l1_gas_prices: header.gas_prices,
            sequencer_address: header.sequencer_address,
        }))
    }
}
