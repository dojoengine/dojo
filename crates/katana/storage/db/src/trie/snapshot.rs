use core::fmt;
use std::marker::PhantomData;

use katana_trie::bonsai::{self, ByteVec, DatabaseKey};
use katana_trie::CommitId;

use crate::abstraction::DbTxRef;
use crate::tables::Trie;
use crate::trie::Error;

#[derive(Debug)]
pub struct SnapshotTrieDb<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'tx>,
{
    tx: Tx,
    snapshot_id: CommitId,
    _table: &'tx PhantomData<Tb>,
}

impl<'tx, Tb, Tx> SnapshotTrieDb<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'tx>,
{
    pub fn new(tx: Tx, id: CommitId) -> Self {
        Self { tx, snapshot_id: id, _table: &PhantomData }
    }
}

impl<'tx, Tb, Tx> bonsai::BonsaiDatabase for SnapshotTrieDb<'tx, Tb, Tx>
where
    Tb: Trie + fmt::Debug,
    Tx: DbTxRef<'tx> + fmt::Debug,
{
    type Batch = ();
    type DatabaseError = Error;

    fn create_batch(&self) -> Self::Batch {}

    fn remove_by_prefix(&mut self, prefix: &DatabaseKey<'_>) -> Result<(), Self::DatabaseError> {
        let _ = prefix;
        unimplemented!("modifying trie snapshot is not supported")
    }

    fn get(&self, key: &DatabaseKey<'_>) -> Result<Option<ByteVec>, Self::DatabaseError> {
        todo!()
    }

    fn get_by_prefix(
        &self,
        prefix: &DatabaseKey,
    ) -> Result<Vec<(ByteVec, ByteVec)>, Self::DatabaseError> {
        todo!()
    }

    fn insert(
        &mut self,
        key: &DatabaseKey<'_>,
        value: &[u8],
        batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let _ = key;
        let _ = value;
        let _ = batch;
        unimplemented!("modifying trie snapshot is not supported")
    }

    fn remove(
        &mut self,
        key: &DatabaseKey<'_>,
        batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let _ = key;
        let _ = batch;
        unimplemented!("modifying trie snapshot is not supported")
    }

    fn contains(&self, key: &DatabaseKey<'_>) -> Result<bool, Self::DatabaseError> {
        todo!()
    }

    fn write_batch(&mut self, batch: Self::Batch) -> Result<(), Self::DatabaseError> {
        let _ = batch;
        unimplemented!("modifying trie snapshot is not supported")
    }
}
