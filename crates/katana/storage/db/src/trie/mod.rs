use core::fmt;
use std::marker::PhantomData;

use anyhow::Result;
use katana_trie::bonsai::{BonsaiDatabase, BonsaiPersistentDatabase, ByteVec, DatabaseKey};
use katana_trie::CommitId;
use smallvec::ToSmallVec;
use snapshot::SnapshotTrieDb;

use crate::abstraction::{DbCursor, DbTxMutRef};
use crate::models::trie::{TrieDatabaseKey, TrieDatabaseKeyType};
use crate::models::{self};
use crate::tables::Trie;

mod snapshot;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] crate::error::DatabaseError);

impl katana_trie::bonsai::DBError for Error {}

#[derive(Debug)]
pub struct TrieDb<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxMutRef<'tx>,
{
    tx: Tx,
    _table: &'tx PhantomData<Tb>,
}

impl<'tx, Tb, Tx> TrieDb<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxMutRef<'tx>,
{
    pub fn new(tx: Tx) -> Self {
        Self { tx, _table: &PhantomData }
    }
}

impl<'tx, Tb, Tx> BonsaiDatabase for TrieDb<'tx, Tb, Tx>
where
    Tb: Trie + fmt::Debug,
    Tx: DbTxMutRef<'tx> + fmt::Debug,
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
        let _ = prefix;
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

    fn write_batch(&mut self, batch: Self::Batch) -> Result<(), Self::DatabaseError> {
        let _ = batch;
        Ok(())
    }
}

impl<'tx, Tb, Tx> BonsaiPersistentDatabase<CommitId> for TrieDb<'tx, Tb, Tx>
where
    Tb: Trie + fmt::Debug,
    Tx: DbTxMutRef<'tx> + fmt::Debug + 'tx,
{
    type DatabaseError = Error;
    type Transaction<'a> = SnapshotTrieDb<'tx, Tb, Tx>  where Self: 'a;

    fn snapshot(&mut self, id: CommitId) {
        let _ = id;
        todo!()
    }

    // merging should recompute the trie again
    fn merge<'a>(&mut self, transaction: Self::Transaction<'a>) -> Result<(), Self::DatabaseError>
    where
        Self: 'a,
    {
        let _ = transaction;
        unimplemented!();
    }

    // TODO: check if the snapshot exist
    fn transaction(&self, id: CommitId) -> Option<(CommitId, Self::Transaction<'_>)> {
        Some((id, SnapshotTrieDb::new(self.tx.clone(), id)))
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
