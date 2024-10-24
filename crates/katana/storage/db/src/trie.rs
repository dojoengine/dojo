use std::marker::PhantomData;

use anyhow::Result;
use katana_trie::bonsai;
use katana_trie::bonsai::id::BasicId;
use katana_trie::bonsai::{BonsaiPersistentDatabase, ByteVec, DatabaseKey};

use crate::abstraction::{DbCursor, DbTxMut};
use crate::models::trie::{TrieDatabaseKey, TrieDatabaseKeyType, TrieDatabaseValue};
use crate::models::{self};
use crate::tables;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] crate::error::DatabaseError);

impl katana_trie::bonsai::DBError for Error {}

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

pub struct TrieDb<Tb: tables::Trie, Tx: DbTxMut> {
    tx: Tx,
    _table: PhantomData<Tb>,
}

impl<Tb, Tx> TrieDb<Tb, Tx>
where
    Tb: tables::Trie,
    Tx: DbTxMut,
{
    pub fn new(tx: Tx) -> Self {
        Self { tx, _table: PhantomData }
    }
}

impl<Tb, Tx> bonsai::BonsaiDatabase for TrieDb<Tb, Tx>
where
    Tb: tables::Trie,
    Tx: DbTxMut,
{
    type Batch = ();
    type DatabaseError = Error;

    fn create_batch(&self) -> Self::Batch {}

    fn remove_by_prefix(&mut self, prefix: &DatabaseKey) -> Result<(), Self::DatabaseError> {
        let mut cursor = self.tx.cursor_mut::<Tb>()?;
        let walker = cursor.walk(None)?;

        let mut keys_to_remove = Vec::new();
        // iterate over all entries in the table
        for entry in walker {
            let (key, _) = entry?;
            if key.key.starts_with(prefix.as_slice()) {
                keys_to_remove.push(key);
            }
        }

        for key in keys_to_remove {
            let _ = self.tx.delete::<Tb>(key, None)?;
        }

        Ok(())
    }

    fn get(&self, key: &DatabaseKey) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let value = self.tx.get::<Tb>(foo(key))?.map(ByteVec::from_const);
        Ok(value)
    }

    fn get_by_prefix(
        &self,
        prefix: &DatabaseKey,
    ) -> Result<Vec<(ByteVec, ByteVec)>, Self::DatabaseError> {
        // let mut result = Vec::new();
        // let db = self.tx.get_map(prefix);
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
        let old_value = self.tx.get::<Tb>(key.clone())?.map(ByteVec::from_const);
        self.tx.put::<Tb>(key, value)?;
        Ok(old_value)
    }

    fn remove(
        &mut self,
        key: &DatabaseKey,
        _batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let key = foo(key);
        let old_value = self.tx.get::<Tb>(key.clone())?.map(ByteVec::from_const);
        self.tx.delete::<Tb>(key, None)?;
        Ok(old_value)
    }

    fn contains(&self, key: &DatabaseKey) -> Result<bool, Self::DatabaseError> {
        let key = foo(key);
        let value = self.tx.get::<Tb>(key)?;
        Ok(value.is_some())
    }

    fn write_batch(&mut self, _batch: Self::Batch) -> Result<(), Self::DatabaseError> {
        Ok(())
    }
}

impl<Tb, Tx> bonsai::BonsaiPersistentDatabase<BasicId> for TrieDb<Tb, Tx>
where
    Tb: tables::Trie,
    Tx: DbTxMut,
{
    type DatabaseError = Error;
    type Transaction = TrieDb<Tb, Tx>;

    fn snapshot(&mut self, _: BasicId) {}

    fn merge(&mut self, _: Self::Transaction) -> Result<(), Self::DatabaseError> {
        Ok(())
    }

    fn transaction(&self, _: BasicId) -> Option<Self::Transaction> {
        None
    }
}
