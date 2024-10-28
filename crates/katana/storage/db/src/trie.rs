use std::marker::PhantomData;

use anyhow::Result;
use katana_trie::bonsai;
use katana_trie::bonsai::id::BasicId;
use katana_trie::bonsai::{ByteVec, DatabaseKey};
use smallvec::ToSmallVec;

use crate::abstraction::{DbCursor, DbTxMut};
use crate::models::trie::{TrieDatabaseKey, TrieDatabaseKeyType};
use crate::models::{self};
use crate::tables;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] crate::error::DatabaseError);

impl katana_trie::bonsai::DBError for Error {}

#[derive(Debug)]
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

    fn remove_by_prefix(&mut self, prefix: &DatabaseKey<'_>) -> Result<(), Self::DatabaseError> {
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

    fn get(&self, key: &DatabaseKey<'_>) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let value = self.tx.get::<Tb>(to_db_key(key))?;
        Ok(value)
    }

    fn get_by_prefix(
        &self,
        prefix: &DatabaseKey<'_>,
    ) -> Result<Vec<(ByteVec, ByteVec)>, Self::DatabaseError> {
        todo!()
    }

    fn insert(
        &mut self,
        key: &DatabaseKey<'_>,
        value: &[u8],
        _batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let key = to_db_key(key);
        let value: ByteVec = value.to_smallvec();
        let old_value = self.tx.get::<Tb>(key.clone())?;
        self.tx.put::<Tb>(key, value)?;
        Ok(old_value)
    }

    fn remove(
        &mut self,
        key: &DatabaseKey<'_>,
        _batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let key = to_db_key(key);
        let old_value = self.tx.get::<Tb>(key.clone())?;
        self.tx.delete::<Tb>(key, None)?;
        Ok(old_value)
    }

    fn contains(&self, key: &DatabaseKey<'_>) -> Result<bool, Self::DatabaseError> {
        let key = to_db_key(key);
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

fn to_db_key(key: &DatabaseKey<'_>) -> models::trie::TrieDatabaseKey {
    match key {
        DatabaseKey::Flat(bytes) => {
            TrieDatabaseKey { key: bytes.to_vec(), r#type: TrieDatabaseKeyType::Flat }
        }
        DatabaseKey::Trie(bytes) => {
            TrieDatabaseKey { key: bytes.to_vec(), r#type: TrieDatabaseKeyType::Trie }
        }
        DatabaseKey::TrieLog(bytes) => {
            TrieDatabaseKey { key: bytes.to_vec(), r#type: TrieDatabaseKeyType::TrieLog }
        }
    }
}
