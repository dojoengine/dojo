use std::collections::BTreeMap;

use anyhow::Result;
use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;
use bonsai_trie::id::BasicId;
use bonsai_trie::{BonsaiStorage, BonsaiStorageConfig, ByteVec, DatabaseKey};
use katana_db::abstraction::DbTxMut;
use katana_db::models::trie::{TrieDatabaseKey, TrieDatabaseKeyType, TrieDatabaseValue};
use katana_db::models::{self};
use katana_db::tables;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::Felt;
use starknet::macros::short_string;
use starknet_types_core::hash::{Poseidon, StarkHash};

// https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-state/#classes_trie
const CONTRACT_CLASS_LEAF_V0: Felt = short_string!("CONTRACT_CLASS_LEAF_V0");

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] katana_db::error::DatabaseError);

impl bonsai_trie::DBError for Error {}

pub struct ClassTrie<Tx: DbTxMut> {
    bonsai_storage: BonsaiStorage<BasicId, TrieDb<Tx>, Poseidon>,
}

impl<Tx: DbTxMut> ClassTrie<Tx> {
    const IDENTIFIER: &'static [u8] = b"0xclass";

    pub fn new(tx: Tx) -> Self {
        let db = TrieDb { tx };
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };
        Self { bonsai_storage: BonsaiStorage::new(db, config).unwrap() }
    }

    pub fn apply(
        &mut self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> Felt {
        let updates: Vec<_> = updates
            .into_iter()
            .map(|(class_hash, compiled_class_hash)| {
                let hash = Poseidon::hash(&CONTRACT_CLASS_LEAF_V0, compiled_class_hash);
                (*class_hash, hash)
            })
            .collect();

        for (key, value) in updates {
            let bytes = key.to_bytes_be();
            let bv: BitVec<u8, Msb0> = bytes.as_bits()[5..].to_owned();
            self.bonsai_storage.insert(Self::IDENTIFIER, &bv, &value).unwrap();
        }

        self.bonsai_storage.commit(BasicId::new(block_number)).unwrap();
        let root_hash = self.bonsai_storage.root_hash(Self::IDENTIFIER).unwrap();
        root_hash
    }
}

fn foo(key: &DatabaseKey) -> models::trie::TrieDatabaseKey {
    match key {
        DatabaseKey::Flat(bytes) => {
            let key = unsafe { *(bytes.as_ptr() as *const [u8; 32]) };
            TrieDatabaseKey { key, r#type: TrieDatabaseKeyType::Flat }
        }
        DatabaseKey::Trie(bytes) => {
            let key = unsafe { *(bytes.as_ptr() as *const [u8; 32]) };
            TrieDatabaseKey { key, r#type: TrieDatabaseKeyType::Trie }
        }
        DatabaseKey::TrieLog(bytes) => {
            let key = unsafe { *(bytes.as_ptr() as *const [u8; 32]) };
            TrieDatabaseKey { key, r#type: TrieDatabaseKeyType::TrieLog }
        }
    }
}

pub struct TrieDb<Tx: DbTxMut> {
    tx: Tx,
}

impl<Tx: DbTxMut> bonsai_trie::BonsaiDatabase for TrieDb<Tx> {
    type Batch = ();
    type DatabaseError = Error;

    fn create_batch(&self) -> Self::Batch {}

    fn remove_by_prefix(&mut self, prefix: &DatabaseKey) -> Result<(), Self::DatabaseError> {
        // let mut cursor = self.tx.cursor_mut::<tables::ClassTrie>()?;
        // let mut walker = cursor.walk(None)?;

        // // iterate over all entries in the table
        // for entry in walker {
        //     let (key, value) = entry?;
        //     if key.key.starts_with(prefix.as_slice()) {
        //         walker.delete_current()?;
        //     }
        // }

        // // let mut keys_to_remove = Vec::new();
        // // for key in db.keys() {
        // //     if key.starts_with(prefix.as_slice()) {
        // //         keys_to_remove.push(key.clone());
        // //     }
        // // }
        // // for key in keys_to_remove {
        // //     db.remove(&key);
        // // }

        todo!()
    }

    fn get(&self, key: &DatabaseKey) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let value = self.tx.get::<tables::ClassTrie>(foo(key))?.map(ByteVec::from_const);
        Ok(value)
    }

    fn get_by_prefix(
        &self,
        prefix: &DatabaseKey,
    ) -> Result<Vec<(ByteVec, ByteVec)>, Self::DatabaseError> {
        // let mut result = Vec::new();
        // let db = self.get_map(prefix);
        // for (key, value) in db.iter() {
        //     if key.starts_with(prefix.as_slice()) {
        //         result.push((key.clone(), value.clone()));
        //     }
        // }
        // Ok(result)

        todo!()
    }

    fn insert(
        &mut self,
        key: &DatabaseKey,
        value: &[u8],
        _batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let key = foo(key);
        let value = unsafe { *(value.as_ptr() as *const TrieDatabaseValue) };
        let old_value = self.tx.get::<tables::ClassTrie>(key.clone())?.map(ByteVec::from_const);
        self.tx.put::<tables::ClassTrie>(key, value)?;
        Ok(old_value)
    }

    fn remove(
        &mut self,
        key: &DatabaseKey,
        _batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let key = foo(key);
        let old_value = self.tx.get::<tables::ClassTrie>(key.clone())?.map(ByteVec::from_const);
        self.tx.delete::<tables::ClassTrie>(key, None)?;
        Ok(old_value)
    }

    fn contains(&self, key: &DatabaseKey) -> Result<bool, Self::DatabaseError> {
        let key = foo(key);
        let value = self.tx.get::<tables::ClassTrie>(key)?;
        Ok(value.is_some())
    }

    fn write_batch(&mut self, _batch: Self::Batch) -> Result<(), Self::DatabaseError> {
        Ok(())
    }
}

impl<Tx: DbTxMut> bonsai_trie::BonsaiPersistentDatabase<BasicId> for TrieDb<Tx> {
    type DatabaseError = Error;
    type Transaction = TrieDb<Tx>;

    fn snapshot(&mut self, _: BasicId) {}

    fn merge(&mut self, _: Self::Transaction) -> Result<(), Self::DatabaseError> {
        Ok(())
    }

    fn transaction(&self, _: BasicId) -> Option<Self::Transaction> {
        None
    }
}
