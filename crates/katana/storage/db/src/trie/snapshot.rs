use core::fmt;
use std::marker::PhantomData;

use anyhow::Result;
use katana_primitives::block::BlockNumber;
use katana_trie::bonsai::{BonsaiDatabase, ByteVec, DatabaseKey};
use katana_trie::CommitId;

use super::Error;
use crate::abstraction::{DbDupSortCursor, DbTxRef};
use crate::models::list::BlockList;
use crate::tables::Trie;
use crate::trie::to_db_key;

pub struct SnapshotTrieDb<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'tx>,
{
    tx: Tx,
    snapshot_id: CommitId,
    _table: &'tx PhantomData<Tb>,
}

impl<'a, Tb, Tx> fmt::Debug for SnapshotTrieDb<'a, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'a> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnapshotTrieDb").field("tx", &self.tx).finish()
    }
}

/// This is a helper function for getting the block number of the most
/// recent change that occurred relative to the given block number.
///
/// ## Arguments
///
/// * `block_list`: A list of block numbers where a change in value occur.
fn recent_change_from_block(target: BlockNumber, block_list: &BlockList) -> Option<BlockNumber> {
    // if the rank is 0, then it's either;
    // 1. the list is empty
    // 2. there are no prior changes occured before/at `block_number`
    let rank = block_list.rank(target);
    if rank == 0 { None } else { block_list.select(rank - 1) }
}

impl<'tx, Tb, Tx> SnapshotTrieDb<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'tx>,
{
    pub(crate) fn new(tx: Tx, id: CommitId) -> Self {
        Self { tx, snapshot_id: id, _table: &PhantomData }
    }
}

impl<'tx, Tb, Tx> BonsaiDatabase for SnapshotTrieDb<'tx, Tb, Tx>
where
    Tb: Trie,
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
        let key = to_db_key(key);
        let block_number = self.snapshot_id.into();

        let change_set = self.tx.get::<Tb::Changeset>(key.clone())?;
        if let Some(num) = change_set.and_then(|set| recent_change_from_block(block_number, &set)) {
            let mut cursor = self.tx.cursor_dup::<Tb::History>()?;
            let entry = cursor
                .seek_by_key_subkey(num, key.clone())?
                .expect("entry should exist if in change set");

            if entry.key == key {
                return Ok(Some(entry.value));
            }
        }

        Ok(None)
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
