pub mod state;

use std::ops::{Range, RangeInclusive};

use anyhow::Result;
use katana_db::mdbx::DbEnv;
use katana_db::models::block::StoredBlockBodyIndices;
use katana_db::models::storage::StorageEntry;
use katana_db::tables::{
    BlockBodyIndices, BlockHashes, BlockNumbers, BlockStatusses, CompiledClassHashes,
    CompiledContractClasses, ContractInfo, ContractStorage, Headers, Receipts, SierraClasses,
    Transactions, TxBlocks, TxHashes, TxNumbers,
};
use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, BlockNumber, BlockWithTxHashes, FinalityStatus, Header,
    SealedBlockWithStatus,
};
use katana_primitives::contract::GenericContractInfo;
use katana_primitives::receipt::Receipt;
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::transaction::{TxHash, TxNumber, TxWithHash};
use katana_primitives::FieldElement;

use crate::traits::block::{
    BlockHashProvider, BlockNumberProvider, BlockProvider, BlockStatusProvider, BlockWriter,
    HeaderProvider,
};
use crate::traits::state::{StateFactoryProvider, StateProvider, StateRootProvider};
use crate::traits::state_update::StateUpdateProvider;
use crate::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};

impl StateFactoryProvider for DbEnv {
    fn latest(&self) -> Result<Box<dyn StateProvider>> {
        let tx = self.tx()?;
        Ok(Box::new(self::state::LatestStateProvider(tx)))
    }

    fn historical(&self, _block_id: BlockHashOrNumber) -> Result<Option<Box<dyn StateProvider>>> {
        unimplemented!("historical state not implemented yet")
    }
}

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

impl StateRootProvider for DbEnv {
    fn state_root(&self, block_id: BlockHashOrNumber) -> Result<Option<FieldElement>> {
        let db_tx = self.tx()?;

        let block_num = match block_id {
            BlockHashOrNumber::Num(num) => Some(num),
            BlockHashOrNumber::Hash(hash) => db_tx.get::<BlockNumbers>(hash)?,
        };

        if let Some(block_num) = block_num {
            let header = db_tx.get::<Headers>(block_num)?;
            db_tx.commit()?;
            Ok(header.map(|h| h.state_root))
        } else {
            Ok(None)
        }
    }
}

impl StateUpdateProvider for DbEnv {
    fn state_update(&self, _block_id: BlockHashOrNumber) -> Result<Option<StateUpdates>> {
        unimplemented!()
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

impl BlockWriter for DbEnv {
    fn insert_block_with_states_and_receipts(
        &self,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithDeclaredClasses,
        receipts: Vec<Receipt>,
    ) -> Result<()> {
        self.update(move |db_tx| -> Result<()> {
            let block_hash = block.block.header.hash;
            let block_number = block.block.header.header.number;

            let block_header = block.block.header.header;
            let transactions = block.block.body;

            let tx_count = transactions.len() as u64;
            let tx_offset = db_tx.entries::<Transactions>()? as u64;
            let block_body_indices = StoredBlockBodyIndices { tx_offset, tx_count };

            db_tx.put::<BlockHashes>(block_number, block_hash)?;
            db_tx.put::<BlockNumbers>(block_hash, block_number)?;
            db_tx.put::<BlockStatusses>(block_number, block.status)?;

            db_tx.put::<Headers>(block_number, block_header)?;
            db_tx.put::<BlockBodyIndices>(block_number, block_body_indices)?;

            for (i, (transaction, receipt)) in transactions.into_iter().zip(receipts).enumerate() {
                let tx_number = tx_offset + i as u64;
                let tx_hash = transaction.hash;

                db_tx.put::<TxHashes>(tx_number, tx_hash)?;
                db_tx.put::<TxNumbers>(tx_hash, tx_number)?;
                db_tx.put::<TxBlocks>(tx_number, block_number)?;
                db_tx.put::<Transactions>(tx_number, transaction.transaction)?;
                db_tx.put::<Receipts>(tx_number, receipt)?;
            }

            // insert classes

            for (class_hash, compiled_hash) in states.state_updates.declared_classes {
                db_tx.put::<CompiledClassHashes>(class_hash, compiled_hash)?;
            }

            for (hash, compiled_class) in states.declared_compiled_classes {
                db_tx.put::<CompiledContractClasses>(hash, compiled_class.into())?;
            }

            for (class_hash, sierra_class) in states.declared_sierra_classes {
                db_tx.put::<SierraClasses>(class_hash, sierra_class)?;
            }

            // insert storage changes
            {
                let mut cursor = db_tx.cursor::<ContractStorage>()?;
                for (addr, entries) in states.state_updates.storage_updates {
                    let entries =
                        entries.into_iter().map(|(key, value)| StorageEntry { key, value });

                    for entry in entries {
                        match cursor.seek_by_key_subkey(addr, entry.key)? {
                            Some(current) if current.key == entry.key => {
                                cursor.delete_current()?;
                            }
                            _ => {}
                        }

                        cursor.upsert(addr, entry)?
                    }
                }
            }

            // update contract info

            for (addr, class_hash) in states.state_updates.contract_updates {
                let value = if let Some(info) = db_tx.get::<ContractInfo>(addr)? {
                    GenericContractInfo { class_hash, ..info }
                } else {
                    GenericContractInfo { class_hash, ..Default::default() }
                };
                db_tx.put::<ContractInfo>(addr, value)?;
            }

            for (addr, nonce) in states.state_updates.nonce_updates {
                let value = if let Some(info) = db_tx.get::<ContractInfo>(addr)? {
                    GenericContractInfo { nonce, ..info }
                } else {
                    GenericContractInfo { nonce, ..Default::default() }
                };
                db_tx.put::<ContractInfo>(addr, value)?;
            }

            Ok(())
        })?
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use katana_db::mdbx::DbEnvKind;
    use katana_primitives::block::{
        Block, BlockHashOrNumber, FinalityStatus, Header, SealedBlockWithStatus,
    };
    use katana_primitives::contract::ContractAddress;
    use katana_primitives::receipt::Receipt;
    use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
    use katana_primitives::transaction::{Tx, TxHash, TxWithHash};
    use starknet::macros::felt;

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
                transaction: Tx::Invoke(Default::default()),
            }],
        }
        .seal();
        SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 }
    }

    fn create_dummy_state_updates() -> StateUpdatesWithDeclaredClasses {
        StateUpdatesWithDeclaredClasses {
            state_updates: StateUpdates {
                nonce_updates: HashMap::from([
                    (ContractAddress::from(felt!("1")), felt!("1")),
                    (ContractAddress::from(felt!("2")), felt!("2")),
                ]),
                contract_updates: HashMap::from([
                    (ContractAddress::from(felt!("1")), felt!("3")),
                    (ContractAddress::from(felt!("2")), felt!("4")),
                ]),
                declared_classes: HashMap::from([
                    (felt!("3"), felt!("89")),
                    (felt!("4"), felt!("90")),
                ]),
                storage_updates: HashMap::from([(
                    ContractAddress::from(felt!("1")),
                    HashMap::from([(felt!("1"), felt!("1")), (felt!("2"), felt!("2"))]),
                )]),
            },
            ..Default::default()
        }
    }

    fn create_dummy_state_updates_2() -> StateUpdatesWithDeclaredClasses {
        StateUpdatesWithDeclaredClasses {
            state_updates: StateUpdates {
                nonce_updates: HashMap::from([
                    (ContractAddress::from(felt!("1")), felt!("5")),
                    (ContractAddress::from(felt!("2")), felt!("6")),
                ]),
                contract_updates: HashMap::from([
                    (ContractAddress::from(felt!("1")), felt!("77")),
                    (ContractAddress::from(felt!("2")), felt!("66")),
                ]),
                storage_updates: HashMap::from([(
                    ContractAddress::from(felt!("1")),
                    HashMap::from([(felt!("1"), felt!("100")), (felt!("2"), felt!("200"))]),
                )]),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn insert_block() {
        let env = katana_db::mdbx::test_utils::create_test_db(DbEnvKind::RW);

        let block = create_dummy_block();
        let state_updates = create_dummy_state_updates();

        // insert block
        BlockWriter::insert_block_with_states_and_receipts(
            &env,
            block.clone(),
            state_updates,
            vec![Receipt::Invoke(Default::default())],
        )
        .expect("failed to insert block");

        // get values

        let block_id: BlockHashOrNumber = block.block.header.hash.into();

        let latest_number = env.latest_number().unwrap();
        let latest_hash = env.latest_hash().unwrap();

        let actual_block = env.block(block_id).unwrap().unwrap();
        let tx_count = env.transaction_count_by_block(block_id).unwrap().unwrap();
        let block_status = env.block_status(block_id).unwrap().unwrap();
        let body_indices = env.block_body_indices(block_id).unwrap().unwrap();

        let tx_hash: TxHash = 24u8.into();
        let tx = env.transaction_by_hash(tx_hash).unwrap().unwrap();

        let state_prov = StateFactoryProvider::latest(&env).unwrap();

        let nonce1 = state_prov.nonce(ContractAddress::from(felt!("1"))).unwrap().unwrap();
        let nonce2 = state_prov.nonce(ContractAddress::from(felt!("2"))).unwrap().unwrap();

        let class_hash1 = state_prov.class_hash_of_contract(felt!("1").into()).unwrap().unwrap();
        let class_hash2 = state_prov.class_hash_of_contract(felt!("2").into()).unwrap().unwrap();

        let compiled_hash1 =
            state_prov.compiled_class_hash_of_class_hash(class_hash1).unwrap().unwrap();
        let compiled_hash2 =
            state_prov.compiled_class_hash_of_class_hash(class_hash2).unwrap().unwrap();

        let storage1 =
            state_prov.storage(ContractAddress::from(felt!("1")), felt!("1")).unwrap().unwrap();
        let storage2 =
            state_prov.storage(ContractAddress::from(felt!("1")), felt!("2")).unwrap().unwrap();

        // assert values are populated correctly

        assert_eq!(tx_hash, tx.hash);
        assert_eq!(tx.transaction, Tx::Invoke(Default::default()));

        assert_eq!(tx_count, 1);
        assert_eq!(body_indices.tx_offset, 0);
        assert_eq!(body_indices.tx_count, tx_count);

        assert_eq!(block_status, FinalityStatus::AcceptedOnL2);
        assert_eq!(block.block.header.hash, latest_hash);
        assert_eq!(block.block.body.len() as u64, tx_count);
        assert_eq!(block.block.header.header.number, latest_number);
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
        let env = katana_db::mdbx::test_utils::create_test_db(DbEnvKind::RW);

        let block = create_dummy_block();
        let state_updates1 = create_dummy_state_updates();
        let state_updates2 = create_dummy_state_updates_2();

        // insert block
        BlockWriter::insert_block_with_states_and_receipts(
            &env,
            block.clone(),
            state_updates1,
            vec![Receipt::Invoke(Default::default())],
        )
        .expect("failed to insert block");

        // insert another block
        BlockWriter::insert_block_with_states_and_receipts(
            &env,
            block.clone(),
            state_updates2,
            vec![Receipt::Invoke(Default::default())],
        )
        .expect("failed to insert block");

        // assert storage is updated correctly

        let state_prov = StateFactoryProvider::latest(&env).unwrap();

        let nonce1 = state_prov.nonce(ContractAddress::from(felt!("1"))).unwrap().unwrap();
        let nonce2 = state_prov.nonce(ContractAddress::from(felt!("2"))).unwrap().unwrap();

        let class_hash1 = state_prov.class_hash_of_contract(felt!("1").into()).unwrap().unwrap();
        let class_hash2 = state_prov.class_hash_of_contract(felt!("2").into()).unwrap().unwrap();

        let storage1 =
            state_prov.storage(ContractAddress::from(felt!("1")), felt!("1")).unwrap().unwrap();
        let storage2 =
            state_prov.storage(ContractAddress::from(felt!("1")), felt!("2")).unwrap().unwrap();

        assert_eq!(nonce1, felt!("5"));
        assert_eq!(nonce2, felt!("6"));

        assert_eq!(class_hash1, felt!("77"));
        assert_eq!(class_hash2, felt!("66"));

        assert_eq!(storage1, felt!("100"));
        assert_eq!(storage2, felt!("200"));
    }
}
