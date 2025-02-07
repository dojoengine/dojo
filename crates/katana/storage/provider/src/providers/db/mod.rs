pub mod state;
pub mod trie;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::ops::{Range, RangeInclusive};

use katana_db::abstraction::{Database, DbCursor, DbCursorMut, DbDupSortCursor, DbTx, DbTxMut};
use katana_db::error::DatabaseError;
use katana_db::init_ephemeral_db;
use katana_db::mdbx::DbEnv;
use katana_db::models::block::StoredBlockBodyIndices;
use katana_db::models::contract::{
    ContractClassChange, ContractInfoChangeList, ContractNonceChange,
};
use katana_db::models::list::BlockList;
use katana_db::models::stage::StageCheckpoint;
use katana_db::models::storage::{ContractStorageEntry, ContractStorageKey, StorageEntry};
use katana_db::tables::{self, DupSort, Table};
use katana_db::utils::KeyValue;
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockNumber, BlockWithTxHashes, FinalityStatus, Header,
    SealedBlockWithStatus,
};
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::contract::{
    ContractAddress, GenericContractInfo, Nonce, StorageKey, StorageValue,
};
use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::Receipt;
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{TxHash, TxNumber, TxWithHash};

use crate::error::ProviderError;
use crate::traits::block::{
    BlockHashProvider, BlockNumberProvider, BlockProvider, BlockStatusProvider, BlockWriter,
    HeaderProvider,
};
use crate::traits::env::BlockEnvProvider;
use crate::traits::stage::StageCheckpointProvider;
use crate::traits::state::{StateFactoryProvider, StateProvider};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionTraceProvider,
    TransactionsProviderExt,
};
use crate::ProviderResult;

/// A provider implementation that uses a persistent database as the backend.
// TODO: remove the default generic type
#[derive(Debug, Clone)]
pub struct DbProvider<Db: Database = DbEnv>(Db);

impl<Db: Database> DbProvider<Db> {
    /// Creates a new [`DbProvider`] from the given [`DbEnv`].
    pub fn new(db: Db) -> Self {
        Self(db)
    }
}

impl DbProvider<DbEnv> {
    /// Creates a new [`DbProvider`] using an ephemeral database.
    pub fn new_ephemeral() -> Self {
        let db = init_ephemeral_db().expect("Failed to initialize ephemeral database");
        Self(db)
    }
}

impl<Db: Database> StateFactoryProvider for DbProvider<Db> {
    fn latest(&self) -> ProviderResult<Box<dyn StateProvider>> {
        Ok(Box::new(self::state::LatestStateProvider::new(self.0.tx()?)))
    }

    fn historical(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Box<dyn StateProvider>>> {
        let block_number = match block_id {
            BlockHashOrNumber::Num(num) => {
                let latest_num = self.latest_number()?;

                match num.cmp(&latest_num) {
                    std::cmp::Ordering::Less => Some(num),
                    std::cmp::Ordering::Greater => return Ok(None),
                    std::cmp::Ordering::Equal => return self.latest().map(Some),
                }
            }

            BlockHashOrNumber::Hash(hash) => self.block_number_by_hash(hash)?,
        };

        let Some(num) = block_number else { return Ok(None) };

        Ok(Some(Box::new(self::state::HistoricalStateProvider::new(self.0.tx()?, num))))
    }
}

impl<Db: Database> BlockNumberProvider for DbProvider<Db> {
    fn block_number_by_hash(&self, hash: BlockHash) -> ProviderResult<Option<BlockNumber>> {
        let db_tx = self.0.tx()?;
        let block_num = db_tx.get::<tables::BlockNumbers>(hash)?;
        db_tx.commit()?;
        Ok(block_num)
    }

    fn latest_number(&self) -> ProviderResult<BlockNumber> {
        let db_tx = self.0.tx()?;
        let res = db_tx.cursor::<tables::BlockHashes>()?.last()?.map(|(num, _)| num);
        let total_blocks = res.ok_or(ProviderError::MissingLatestBlockNumber)?;
        db_tx.commit()?;
        Ok(total_blocks)
    }
}

impl<Db: Database> BlockHashProvider for DbProvider<Db> {
    fn latest_hash(&self) -> ProviderResult<BlockHash> {
        let latest_block = self.latest_number()?;
        let db_tx = self.0.tx()?;
        let latest_hash = db_tx.get::<tables::BlockHashes>(latest_block)?;
        db_tx.commit()?;
        latest_hash.ok_or(ProviderError::MissingLatestBlockHash)
    }

    fn block_hash_by_num(&self, num: BlockNumber) -> ProviderResult<Option<BlockHash>> {
        let db_tx = self.0.tx()?;
        let block_hash = db_tx.get::<tables::BlockHashes>(num)?;
        db_tx.commit()?;
        Ok(block_hash)
    }
}

impl<Db: Database> HeaderProvider for DbProvider<Db> {
    fn header(&self, id: BlockHashOrNumber) -> ProviderResult<Option<Header>> {
        let db_tx = self.0.tx()?;

        let num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<tables::BlockNumbers>(hash)?,
        };

        if let Some(num) = num {
            let header =
                db_tx.get::<tables::Headers>(num)?.ok_or(ProviderError::MissingBlockHeader(num))?;
            db_tx.commit()?;
            Ok(Some(header))
        } else {
            Ok(None)
        }
    }
}

impl<Db: Database> BlockProvider for DbProvider<Db> {
    fn block_body_indices(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<StoredBlockBodyIndices>> {
        let db_tx = self.0.tx()?;

        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<tables::BlockNumbers>(hash)?,
        };

        if let Some(num) = block_num {
            let indices = db_tx.get::<tables::BlockBodyIndices>(num)?;
            db_tx.commit()?;
            Ok(indices)
        } else {
            Ok(None)
        }
    }

    fn block(&self, id: BlockHashOrNumber) -> ProviderResult<Option<Block>> {
        let db_tx = self.0.tx()?;

        if let Some(header) = self.header(id)? {
            let res = self.transactions_by_block(id)?;
            let body = res.ok_or(ProviderError::MissingBlockTxs(header.number))?;

            db_tx.commit()?;
            Ok(Some(Block { header, body }))
        } else {
            Ok(None)
        }
    }

    fn block_with_tx_hashes(
        &self,
        id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BlockWithTxHashes>> {
        let db_tx = self.0.tx()?;

        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<tables::BlockNumbers>(hash)?,
        };

        let Some(block_num) = block_num else { return Ok(None) };

        if let Some(header) = db_tx.get::<tables::Headers>(block_num)? {
            let res = db_tx.get::<tables::BlockBodyIndices>(block_num)?;
            let body_indices = res.ok_or(ProviderError::MissingBlockTxs(block_num))?;

            let body = self.transaction_hashes_in_range(Range::from(body_indices))?;
            let block = BlockWithTxHashes { header, body };

            db_tx.commit()?;

            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    fn blocks_in_range(&self, range: RangeInclusive<u64>) -> ProviderResult<Vec<Block>> {
        let db_tx = self.0.tx()?;

        let total = range.end() - range.start() + 1;
        let mut blocks = Vec::with_capacity(total as usize);

        for num in range {
            if let Some(header) = db_tx.get::<tables::Headers>(num)? {
                let res = db_tx.get::<tables::BlockBodyIndices>(num)?;
                let body_indices = res.ok_or(ProviderError::MissingBlockBodyIndices(num))?;

                let body = self.transaction_in_range(Range::from(body_indices))?;
                blocks.push(Block { header, body })
            }
        }

        db_tx.commit()?;
        Ok(blocks)
    }
}

impl<Db: Database> BlockStatusProvider for DbProvider<Db> {
    fn block_status(&self, id: BlockHashOrNumber) -> ProviderResult<Option<FinalityStatus>> {
        let db_tx = self.0.tx()?;

        let block_num = match id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => self.block_number_by_hash(hash)?,
        };

        if let Some(block_num) = block_num {
            let res = db_tx.get::<tables::BlockStatusses>(block_num)?;
            let status = res.ok_or(ProviderError::MissingBlockStatus(block_num))?;

            db_tx.commit()?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }
}

// A helper function that iterates over all entries in a dupsort table and collects the
// results into `V`. If `key` is not found, `V::default()` is returned.
fn dup_entries<Db, Tb, V, T>(
    db_tx: &<Db as Database>::Tx,
    key: <Tb as Table>::Key,
    mut f: impl FnMut(Result<KeyValue<Tb>, DatabaseError>) -> ProviderResult<Option<T>>,
) -> ProviderResult<V>
where
    Db: Database,
    Tb: DupSort + Debug,
    V: FromIterator<T> + Default,
{
    Ok(db_tx
        .cursor_dup::<Tb>()?
        .walk_dup(Some(key), None)?
        .map(|walker| walker.filter_map(|i| f(i).transpose()).collect::<ProviderResult<V>>())
        .transpose()?
        .unwrap_or_default())
}

impl<Db: Database> StateUpdateProvider for DbProvider<Db> {
    fn state_update(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<StateUpdates>> {
        let db_tx = self.0.tx()?;
        let block_num = self.block_number_by_id(block_id)?;

        if let Some(block_num) = block_num {
            let nonce_updates = dup_entries::<
                Db,
                tables::NonceChangeHistory,
                BTreeMap<ContractAddress, Nonce>,
                _,
            >(&db_tx, block_num, |entry| {
                let (_, ContractNonceChange { contract_address, nonce }) = entry?;
                Ok(Some((contract_address, nonce)))
            })?;

            let deployed_contracts = dup_entries::<
                Db,
                tables::ClassChangeHistory,
                BTreeMap<ContractAddress, ClassHash>,
                _,
            >(&db_tx, block_num, |entry| {
                let (_, ContractClassChange { contract_address, class_hash }) = entry?;
                Ok(Some((contract_address, class_hash)))
            })?;

            let mut declared_classes = BTreeMap::new();
            let mut deprecated_declared_classes = BTreeSet::new();

            if let Some(block_entries) =
                db_tx.cursor_dup::<tables::ClassDeclarations>()?.walk_dup(Some(block_num), None)?
            {
                for entry in block_entries {
                    let (_, class_hash) = entry?;
                    match db_tx.get::<tables::CompiledClassHashes>(class_hash)? {
                        Some(compiled_hash) => {
                            declared_classes.insert(class_hash, compiled_hash);
                        }
                        None => {
                            deprecated_declared_classes.insert(class_hash);
                        }
                    }
                }
            }

            let storage_updates = {
                let entries = dup_entries::<
                    Db,
                    tables::StorageChangeHistory,
                    Vec<(ContractAddress, (StorageKey, StorageValue))>,
                    _,
                >(&db_tx, block_num, |entry| {
                    let (_, ContractStorageEntry { key, value }) = entry?;
                    Ok(Some((key.contract_address, (key.key, value))))
                })?;

                let mut map: BTreeMap<_, BTreeMap<StorageKey, StorageValue>> = BTreeMap::new();

                entries.into_iter().for_each(|(addr, (key, value))| {
                    map.entry(addr).or_default().insert(key, value);
                });

                map
            };

            db_tx.commit()?;

            Ok(Some(StateUpdates {
                nonce_updates,
                storage_updates,
                deployed_contracts,
                declared_classes,
                deprecated_declared_classes,
                replaced_classes: BTreeMap::default(),
            }))
        } else {
            Ok(None)
        }
    }

    fn declared_classes(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BTreeMap<ClassHash, CompiledClassHash>>> {
        let db_tx = self.0.tx()?;
        let block_num = self.block_number_by_id(block_id)?;

        if let Some(block_num) = block_num {
            let declared_classes = dup_entries::<
                Db,
                tables::ClassDeclarations,
                BTreeMap<ClassHash, CompiledClassHash>,
                _,
            >(&db_tx, block_num, |entry| {
                let (_, class_hash) = entry?;

                if let Some(compiled_hash) = db_tx.get::<tables::CompiledClassHashes>(class_hash)? {
                    Ok(Some((class_hash, compiled_hash)))
                } else {
                    Ok(None)
                }
            })?;

            db_tx.commit()?;
            Ok(Some(declared_classes))
        } else {
            Ok(None)
        }
    }

    fn deployed_contracts(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BTreeMap<ContractAddress, ClassHash>>> {
        let db_tx = self.0.tx()?;
        let block_num = self.block_number_by_id(block_id)?;

        if let Some(block_num) = block_num {
            let deployed_contracts = dup_entries::<
                Db,
                tables::ClassChangeHistory,
                BTreeMap<ContractAddress, ClassHash>,
                _,
            >(&db_tx, block_num, |entry| {
                let (_, ContractClassChange { contract_address, class_hash }) = entry?;
                Ok(Some((contract_address, class_hash)))
            })?;

            db_tx.commit()?;
            Ok(Some(deployed_contracts))
        } else {
            Ok(None)
        }
    }
}

impl<Db: Database> TransactionProvider for DbProvider<Db> {
    fn transaction_by_hash(&self, hash: TxHash) -> ProviderResult<Option<TxWithHash>> {
        let db_tx = self.0.tx()?;

        if let Some(num) = db_tx.get::<tables::TxNumbers>(hash)? {
            let res = db_tx.get::<tables::Transactions>(num)?;
            let transaction = res.ok_or(ProviderError::MissingTx(num))?;
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
    ) -> ProviderResult<Option<Vec<TxWithHash>>> {
        if let Some(indices) = self.block_body_indices(block_id)? {
            Ok(Some(self.transaction_in_range(Range::from(indices))?))
        } else {
            Ok(None)
        }
    }

    fn transaction_in_range(&self, range: Range<TxNumber>) -> ProviderResult<Vec<TxWithHash>> {
        let db_tx = self.0.tx()?;

        let total = range.end - range.start;
        let mut transactions = Vec::with_capacity(total as usize);

        for i in range {
            if let Some(transaction) = db_tx.get::<tables::Transactions>(i)? {
                let res = db_tx.get::<tables::TxHashes>(i)?;
                let hash = res.ok_or(ProviderError::MissingTxHash(i))?;

                transactions.push(TxWithHash { hash, transaction });
            };
        }

        db_tx.commit()?;
        Ok(transactions)
    }

    fn transaction_block_num_and_hash(
        &self,
        hash: TxHash,
    ) -> ProviderResult<Option<(BlockNumber, BlockHash)>> {
        let db_tx = self.0.tx()?;
        if let Some(num) = db_tx.get::<tables::TxNumbers>(hash)? {
            let block_num =
                db_tx.get::<tables::TxBlocks>(num)?.ok_or(ProviderError::MissingTxBlock(num))?;

            let res = db_tx.get::<tables::BlockHashes>(block_num)?;
            let block_hash = res.ok_or(ProviderError::MissingBlockHash(num))?;

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
    ) -> ProviderResult<Option<TxWithHash>> {
        let db_tx = self.0.tx()?;

        match self.block_body_indices(block_id)? {
            // make sure the requested idx is within the range of the block tx count
            Some(indices) if idx < indices.tx_count => {
                let num = indices.tx_offset + idx;

                let res = db_tx.get::<tables::TxHashes>(num)?;
                let hash = res.ok_or(ProviderError::MissingTxHash(num))?;

                let res = db_tx.get::<tables::Transactions>(num)?;
                let transaction = res.ok_or(ProviderError::MissingTx(num))?;

                db_tx.commit()?;
                Ok(Some(TxWithHash { hash, transaction }))
            }

            _ => Ok(None),
        }
    }

    fn transaction_count_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<u64>> {
        let db_tx = self.0.tx()?;
        if let Some(indices) = self.block_body_indices(block_id)? {
            db_tx.commit()?;
            Ok(Some(indices.tx_count))
        } else {
            Ok(None)
        }
    }
}

impl<Db: Database> TransactionsProviderExt for DbProvider<Db> {
    fn transaction_hashes_in_range(&self, range: Range<TxNumber>) -> ProviderResult<Vec<TxHash>> {
        let db_tx = self.0.tx()?;

        let total = range.end - range.start;
        let mut hashes = Vec::with_capacity(total as usize);

        for i in range {
            if let Some(hash) = db_tx.get::<tables::TxHashes>(i)? {
                hashes.push(hash);
            }
        }

        db_tx.commit()?;
        Ok(hashes)
    }
}

impl<Db: Database> TransactionStatusProvider for DbProvider<Db> {
    fn transaction_status(&self, hash: TxHash) -> ProviderResult<Option<FinalityStatus>> {
        let db_tx = self.0.tx()?;
        if let Some(tx_num) = db_tx.get::<tables::TxNumbers>(hash)? {
            let res = db_tx.get::<tables::TxBlocks>(tx_num)?;
            let block_num = res.ok_or(ProviderError::MissingTxBlock(tx_num))?;

            let res = db_tx.get::<tables::BlockStatusses>(block_num)?;
            let status = res.ok_or(ProviderError::MissingBlockStatus(block_num))?;

            db_tx.commit()?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }
}

impl<Db: Database> TransactionTraceProvider for DbProvider<Db> {
    fn transaction_execution(&self, hash: TxHash) -> ProviderResult<Option<TxExecInfo>> {
        let db_tx = self.0.tx()?;
        if let Some(num) = db_tx.get::<tables::TxNumbers>(hash)? {
            let execution = db_tx
                .get::<tables::TxTraces>(num)?
                .ok_or(ProviderError::MissingTxExecution(num))?;

            db_tx.commit()?;
            Ok(Some(execution))
        } else {
            Ok(None)
        }
    }

    fn transaction_executions_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<TxExecInfo>>> {
        if let Some(index) = self.block_body_indices(block_id)? {
            let traces = self.transaction_executions_in_range(index.into())?;
            Ok(Some(traces))
        } else {
            Ok(None)
        }
    }

    fn transaction_executions_in_range(
        &self,
        range: Range<TxNumber>,
    ) -> ProviderResult<Vec<TxExecInfo>> {
        let db_tx = self.0.tx()?;

        let total = range.end - range.start;
        let mut traces = Vec::with_capacity(total as usize);

        for i in range {
            if let Some(trace) = db_tx.get::<tables::TxTraces>(i)? {
                traces.push(trace);
            }
        }

        db_tx.commit()?;
        Ok(traces)
    }
}

impl<Db: Database> ReceiptProvider for DbProvider<Db> {
    fn receipt_by_hash(&self, hash: TxHash) -> ProviderResult<Option<Receipt>> {
        let db_tx = self.0.tx()?;
        if let Some(num) = db_tx.get::<tables::TxNumbers>(hash)? {
            let receipt =
                db_tx.get::<tables::Receipts>(num)?.ok_or(ProviderError::MissingTxReceipt(num))?;

            db_tx.commit()?;
            Ok(Some(receipt))
        } else {
            Ok(None)
        }
    }

    fn receipts_by_block(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Vec<Receipt>>> {
        if let Some(indices) = self.block_body_indices(block_id)? {
            let db_tx = self.0.tx()?;
            let mut receipts = Vec::with_capacity(indices.tx_count as usize);

            let range = indices.tx_offset..indices.tx_offset + indices.tx_count;
            for i in range {
                if let Some(receipt) = db_tx.get::<tables::Receipts>(i)? {
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

impl<Db: Database> BlockEnvProvider for DbProvider<Db> {
    fn block_env_at(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<BlockEnv>> {
        let Some(header) = self.header(block_id)? else { return Ok(None) };

        Ok(Some(BlockEnv {
            number: header.number,
            timestamp: header.timestamp,
            l1_gas_prices: header.l1_gas_prices,
            l1_data_gas_prices: header.l1_data_gas_prices,
            sequencer_address: header.sequencer_address,
        }))
    }
}

impl<Db: Database> BlockWriter for DbProvider<Db> {
    fn insert_block_with_states_and_receipts(
        &self,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithClasses,
        receipts: Vec<Receipt>,
        executions: Vec<TxExecInfo>,
    ) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            let block_hash = block.block.hash;
            let block_number = block.block.header.number;

            let block_header = block.block.header;
            let transactions = block.block.body;

            let tx_count = transactions.len() as u64;
            let tx_offset = db_tx.entries::<tables::Transactions>()? as u64;
            let block_body_indices = StoredBlockBodyIndices { tx_offset, tx_count };

            db_tx.put::<tables::BlockHashes>(block_number, block_hash)?;
            db_tx.put::<tables::BlockNumbers>(block_hash, block_number)?;
            db_tx.put::<tables::BlockStatusses>(block_number, block.status)?;

            db_tx.put::<tables::Headers>(block_number, block_header)?;
            db_tx.put::<tables::BlockBodyIndices>(block_number, block_body_indices)?;

            // Store base transaction details
            for (i, transaction) in transactions.into_iter().enumerate() {
                let tx_number = tx_offset + i as u64;
                let tx_hash = transaction.hash;

                db_tx.put::<tables::TxHashes>(tx_number, tx_hash)?;
                db_tx.put::<tables::TxNumbers>(tx_hash, tx_number)?;
                db_tx.put::<tables::TxBlocks>(tx_number, block_number)?;
                db_tx.put::<tables::Transactions>(tx_number, transaction.transaction)?;
            }

            // Store transaction receipts
            for (i, receipt) in receipts.into_iter().enumerate() {
                let tx_number = tx_offset + i as u64;
                db_tx.put::<tables::Receipts>(tx_number, receipt)?;
            }

            // Store execution traces
            for (i, execution) in executions.into_iter().enumerate() {
                let tx_number = tx_offset + i as u64;
                db_tx.put::<tables::TxTraces>(tx_number, execution)?;
            }

            // insert classes

            for (class_hash, compiled_hash) in states.state_updates.declared_classes {
                db_tx.put::<tables::CompiledClassHashes>(class_hash, compiled_hash)?;

                db_tx.put::<tables::ClassDeclarationBlock>(class_hash, block_number)?;
                db_tx.put::<tables::ClassDeclarations>(block_number, class_hash)?
            }

            for class_hash in states.state_updates.deprecated_declared_classes {
                db_tx.put::<tables::ClassDeclarationBlock>(class_hash, block_number)?;
                db_tx.put::<tables::ClassDeclarations>(block_number, class_hash)?
            }

            for (class_hash, class) in states.classes {
                // generate the compiled class
                let compiled = class.clone().compile()?;
                db_tx.put::<tables::Classes>(class_hash, class)?;
                db_tx.put::<tables::CompiledClasses>(class_hash, compiled)?;
            }

            // insert storage changes
            {
                let mut storage_cursor = db_tx.cursor_dup_mut::<tables::ContractStorage>()?;
                for (addr, entries) in states.state_updates.storage_updates {
                    let entries =
                        entries.into_iter().map(|(key, value)| StorageEntry { key, value });

                    for entry in entries {
                        match storage_cursor.seek_by_key_subkey(addr, entry.key)? {
                            Some(current) if current.key == entry.key => {
                                storage_cursor.delete_current()?;
                            }

                            _ => {}
                        }

                        // update block list in the change set
                        let changeset_key =
                            ContractStorageKey { contract_address: addr, key: entry.key };
                        let list = db_tx.get::<tables::StorageChangeSet>(changeset_key.clone())?;

                        let updated_list = match list {
                            Some(mut list) => {
                                list.insert(block_number);
                                list
                            }
                            // create a new block list if it doesn't yet exist, and insert the block
                            // number
                            None => BlockList::from([block_number]),
                        };

                        db_tx.put::<tables::StorageChangeSet>(changeset_key, updated_list)?;
                        storage_cursor.upsert(addr, entry)?;

                        let storage_change_sharded_key =
                            ContractStorageKey { contract_address: addr, key: entry.key };

                        db_tx.put::<tables::StorageChangeHistory>(
                            block_number,
                            ContractStorageEntry {
                                key: storage_change_sharded_key,
                                value: entry.value,
                            },
                        )?;
                    }
                }
            }

            // update contract info

            for (addr, class_hash) in states.state_updates.deployed_contracts {
                let value = if let Some(info) = db_tx.get::<tables::ContractInfo>(addr)? {
                    GenericContractInfo { class_hash, ..info }
                } else {
                    GenericContractInfo { class_hash, ..Default::default() }
                };

                let new_change_set = if let Some(mut change_set) =
                    db_tx.get::<tables::ContractInfoChangeSet>(addr)?
                {
                    change_set.class_change_list.insert(block_number);
                    change_set
                } else {
                    ContractInfoChangeList {
                        class_change_list: BlockList::from([block_number]),
                        ..Default::default()
                    }
                };

                db_tx.put::<tables::ContractInfo>(addr, value)?;

                let class_change_key = ContractClassChange { contract_address: addr, class_hash };
                db_tx.put::<tables::ClassChangeHistory>(block_number, class_change_key)?;
                db_tx.put::<tables::ContractInfoChangeSet>(addr, new_change_set)?;
            }

            for (addr, nonce) in states.state_updates.nonce_updates {
                let value = if let Some(info) = db_tx.get::<tables::ContractInfo>(addr)? {
                    GenericContractInfo { nonce, ..info }
                } else {
                    GenericContractInfo { nonce, ..Default::default() }
                };

                let new_change_set = if let Some(mut change_set) =
                    db_tx.get::<tables::ContractInfoChangeSet>(addr)?
                {
                    change_set.nonce_change_list.insert(block_number);
                    change_set
                } else {
                    ContractInfoChangeList {
                        nonce_change_list: BlockList::from([block_number]),
                        ..Default::default()
                    }
                };

                db_tx.put::<tables::ContractInfo>(addr, value)?;

                let nonce_change_key = ContractNonceChange { contract_address: addr, nonce };
                db_tx.put::<tables::NonceChangeHistory>(block_number, nonce_change_key)?;
                db_tx.put::<tables::ContractInfoChangeSet>(addr, new_change_set)?;
            }

            Ok(())
        })?
    }
}

impl<Db: Database> StageCheckpointProvider for DbProvider<Db> {
    fn checkpoint(&self, id: &str) -> ProviderResult<Option<BlockNumber>> {
        let tx = self.0.tx()?;
        let result = tx.get::<tables::StageCheckpoints>(id.to_string())?;
        tx.commit()?;
        Ok(result.map(|x| x.block))
    }

    fn set_checkpoint(&self, id: &str, block_number: BlockNumber) -> ProviderResult<()> {
        let tx = self.0.tx_mut()?;

        let key = id.to_string();
        let value = StageCheckpoint { block: block_number };
        tx.put::<tables::StageCheckpoints>(key, value)?;

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use katana_primitives::address;
    use katana_primitives::block::{
        Block, BlockHashOrNumber, FinalityStatus, Header, SealedBlockWithStatus,
    };
    use katana_primitives::contract::ContractAddress;
    use katana_primitives::fee::{PriceUnit, TxFeeInfo};
    use katana_primitives::receipt::{InvokeTxReceipt, Receipt};
    use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
    use katana_primitives::trace::TxExecInfo;
    use katana_primitives::transaction::{InvokeTx, Tx, TxHash, TxWithHash};
    use starknet::macros::felt;

    use super::DbProvider;
    use crate::traits::block::{
        BlockHashProvider, BlockNumberProvider, BlockProvider, BlockStatusProvider, BlockWriter,
    };
    use crate::traits::state::StateFactoryProvider;
    use crate::traits::transaction::TransactionProvider;

    fn create_dummy_block() -> SealedBlockWithStatus {
        let header = Header { parent_hash: 199u8.into(), number: 0, ..Default::default() };
        let block = Block {
            header,
            body: vec![TxWithHash {
                hash: 24u8.into(),
                transaction: Tx::Invoke(InvokeTx::V1(Default::default())),
            }],
        }
        .seal();
        SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 }
    }

    fn create_dummy_state_updates() -> StateUpdatesWithClasses {
        StateUpdatesWithClasses {
            state_updates: StateUpdates {
                nonce_updates: BTreeMap::from([
                    (address!("1"), felt!("1")),
                    (address!("2"), felt!("2")),
                ]),
                deployed_contracts: BTreeMap::from([
                    (address!("1"), felt!("3")),
                    (address!("2"), felt!("4")),
                ]),
                declared_classes: BTreeMap::from([
                    (felt!("3"), felt!("89")),
                    (felt!("4"), felt!("90")),
                ]),
                storage_updates: BTreeMap::from([(
                    address!("1"),
                    BTreeMap::from([(felt!("1"), felt!("1")), (felt!("2"), felt!("2"))]),
                )]),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn create_dummy_state_updates_2() -> StateUpdatesWithClasses {
        StateUpdatesWithClasses {
            state_updates: StateUpdates {
                nonce_updates: BTreeMap::from([
                    (address!("1"), felt!("5")),
                    (address!("2"), felt!("6")),
                ]),
                deployed_contracts: BTreeMap::from([
                    (address!("1"), felt!("77")),
                    (address!("2"), felt!("66")),
                ]),
                storage_updates: BTreeMap::from([(
                    address!("1"),
                    BTreeMap::from([(felt!("1"), felt!("100")), (felt!("2"), felt!("200"))]),
                )]),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn create_db_provider() -> DbProvider {
        DbProvider(katana_db::mdbx::test_utils::create_test_db())
    }

    #[test]
    fn insert_block() {
        let provider = create_db_provider();
        let block = create_dummy_block();
        let state_updates = create_dummy_state_updates();

        // insert block
        BlockWriter::insert_block_with_states_and_receipts(
            &provider,
            block.clone(),
            state_updates,
            vec![Receipt::Invoke(InvokeTxReceipt {
                revert_error: None,
                events: Vec::new(),
                messages_sent: Vec::new(),
                execution_resources: Default::default(),
                fee: TxFeeInfo {
                    gas_consumed: 0,
                    gas_price: 0,
                    overall_fee: 0,
                    unit: PriceUnit::Wei,
                },
            })],
            vec![TxExecInfo::default()],
        )
        .expect("failed to insert block");

        // get values

        let block_id: BlockHashOrNumber = block.block.hash.into();

        let latest_number = provider.latest_number().unwrap();
        let latest_hash = provider.latest_hash().unwrap();

        let actual_block = provider.block(block_id).unwrap().unwrap();
        let tx_count = provider.transaction_count_by_block(block_id).unwrap().unwrap();
        let block_status = provider.block_status(block_id).unwrap().unwrap();
        let body_indices = provider.block_body_indices(block_id).unwrap().unwrap();

        let tx_hash: TxHash = 24u8.into();
        let tx = provider.transaction_by_hash(tx_hash).unwrap().unwrap();

        let state_prov = StateFactoryProvider::latest(&provider).unwrap();

        let nonce1 = state_prov.nonce(address!("1")).unwrap().unwrap();
        let nonce2 = state_prov.nonce(address!("2")).unwrap().unwrap();

        let class_hash1 = state_prov.class_hash_of_contract(felt!("1").into()).unwrap().unwrap();
        let class_hash2 = state_prov.class_hash_of_contract(felt!("2").into()).unwrap().unwrap();

        let compiled_hash1 =
            state_prov.compiled_class_hash_of_class_hash(class_hash1).unwrap().unwrap();
        let compiled_hash2 =
            state_prov.compiled_class_hash_of_class_hash(class_hash2).unwrap().unwrap();

        let storage1 = state_prov.storage(address!("1"), felt!("1")).unwrap().unwrap();
        let storage2 = state_prov.storage(address!("1"), felt!("2")).unwrap().unwrap();

        // assert values are populated correctly

        assert_eq!(tx_hash, tx.hash);
        assert_eq!(tx.transaction, Tx::Invoke(InvokeTx::V1(Default::default())));

        assert_eq!(tx_count, 1);
        assert_eq!(body_indices.tx_offset, 0);
        assert_eq!(body_indices.tx_count, tx_count);

        assert_eq!(block_status, FinalityStatus::AcceptedOnL2);
        assert_eq!(block.block.hash, latest_hash);
        assert_eq!(block.block.body.len() as u64, tx_count);
        assert_eq!(block.block.header.number, latest_number);
        assert_eq!(block.block.unseal(), actual_block);

        assert_eq!(nonce1, felt!("1"));
        assert_eq!(nonce2, felt!("2"));
        assert_eq!(class_hash1, felt!("3"));
        assert_eq!(class_hash2, felt!("4"));

        assert_eq!(compiled_hash1, felt!("89"));
        assert_eq!(compiled_hash2, felt!("90"));

        assert_eq!(storage1, felt!("1"));
        assert_eq!(storage2, felt!("2"));
    }

    #[test]
    fn storage_updated_correctly() {
        let provider = create_db_provider();

        let block = create_dummy_block();
        let state_updates1 = create_dummy_state_updates();
        let state_updates2 = create_dummy_state_updates_2();

        // insert block
        BlockWriter::insert_block_with_states_and_receipts(
            &provider,
            block.clone(),
            state_updates1,
            vec![Receipt::Invoke(InvokeTxReceipt {
                revert_error: None,
                events: Vec::new(),
                messages_sent: Vec::new(),
                execution_resources: Default::default(),
                fee: TxFeeInfo {
                    gas_consumed: 0,
                    gas_price: 0,
                    overall_fee: 0,
                    unit: PriceUnit::Wei,
                },
            })],
            vec![TxExecInfo::default()],
        )
        .expect("failed to insert block");

        // insert another block
        BlockWriter::insert_block_with_states_and_receipts(
            &provider,
            block,
            state_updates2,
            vec![Receipt::Invoke(InvokeTxReceipt {
                revert_error: None,
                events: Vec::new(),
                messages_sent: Vec::new(),
                execution_resources: Default::default(),
                fee: TxFeeInfo {
                    gas_consumed: 0,
                    gas_price: 0,
                    overall_fee: 0,
                    unit: PriceUnit::Wei,
                },
            })],
            vec![TxExecInfo::default()],
        )
        .expect("failed to insert block");

        // assert storage is updated correctly

        let state_prov = StateFactoryProvider::latest(&provider).unwrap();

        let nonce1 = state_prov.nonce(address!("1")).unwrap().unwrap();
        let nonce2 = state_prov.nonce(address!("2")).unwrap().unwrap();

        let class_hash1 = state_prov.class_hash_of_contract(felt!("1").into()).unwrap().unwrap();
        let class_hash2 = state_prov.class_hash_of_contract(felt!("2").into()).unwrap().unwrap();

        let storage1 = state_prov.storage(address!("1"), felt!("1")).unwrap().unwrap();
        let storage2 = state_prov.storage(address!("1"), felt!("2")).unwrap().unwrap();

        assert_eq!(nonce1, felt!("5"));
        assert_eq!(nonce2, felt!("6"));

        assert_eq!(class_hash1, felt!("77"));
        assert_eq!(class_hash2, felt!("66"));

        assert_eq!(storage1, felt!("100"));
        assert_eq!(storage2, felt!("200"));
    }
}
