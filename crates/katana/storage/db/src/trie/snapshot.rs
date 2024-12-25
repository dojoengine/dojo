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

    fn remove_by_prefix(&mut self, _: &DatabaseKey<'_>) -> Result<(), Self::DatabaseError> {
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
        _: &DatabaseKey<'_>,
    ) -> Result<Vec<(ByteVec, ByteVec)>, Self::DatabaseError> {
        todo!()
    }

    fn insert(
        &mut self,
        _: &DatabaseKey<'_>,
        _: &[u8],
        _: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        unimplemented!("modifying trie snapshot is not supported")
    }

    fn remove(
        &mut self,
        _: &DatabaseKey<'_>,
        _: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        unimplemented!("modifying trie snapshot is not supported")
    }

    fn contains(&self, _: &DatabaseKey<'_>) -> Result<bool, Self::DatabaseError> {
        todo!()
    }

    fn write_batch(&mut self, _: Self::Batch) -> Result<(), Self::DatabaseError> {
        unimplemented!("modifying trie snapshot is not supported")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use katana_primitives::felt;
    use katana_trie::bonsai::DatabaseKey;
    use katana_trie::{BonsaiPersistentDatabase, CommitId};
    use proptest::prelude::*;
    use proptest::strategy;

    use super::*;
    use crate::abstraction::{Database, DbTx};
    use crate::mdbx::test_utils;
    use crate::models::trie::TrieDatabaseKeyType;
    use crate::tables;
    use crate::trie::{SnapshotTrieDb, TrieDbMut};

    #[allow(unused)]
    fn arb_db_key_type() -> BoxedStrategy<TrieDatabaseKeyType> {
        prop_oneof![
            Just(TrieDatabaseKeyType::Trie),
            Just(TrieDatabaseKeyType::Flat),
            Just(TrieDatabaseKeyType::TrieLog),
        ]
        .boxed()
    }

    #[derive(Debug)]
    struct Case {
        number: BlockNumber,
        keyvalues: HashMap<(TrieDatabaseKeyType, [u8; 32]), [u8; 32]>,
    }

    prop_compose! {
        // This create a strategy that generates a random values but always a hardcoded key
        fn arb_keyvalues_with_fixed_key() (
            value in any::<[u8;32]>()
        ) -> HashMap<(TrieDatabaseKeyType, [u8; 32]), [u8; 32]> {
            let key = (TrieDatabaseKeyType::Trie, felt!("0x112345678921541231").to_bytes_be());
            HashMap::from_iter([(key, value)])
        }
    }

    prop_compose! {
        fn arb_keyvalues() (
            keyvalues in prop::collection::hash_map(
                (arb_db_key_type(), any::<[u8;32]>()),
                any::<[u8;32]>(),
                1..100
            )
        ) -> HashMap<(TrieDatabaseKeyType, [u8; 32]), [u8; 32]> {
            keyvalues
        }
    }

    prop_compose! {
        fn arb_block(count: u64, step: u64) (
            number in (count * step)..((count * step) + step),
            keyvalues in arb_keyvalues_with_fixed_key()
        ) -> Case {
            Case { number, keyvalues }
        }
    }

    /// Strategy for generating a list of blocks with `count` size where each block is within a
    /// range of `step` size. See [`arb_block`].
    fn arb_blocklist(step: u64, count: usize) -> impl strategy::Strategy<Value = Vec<Case>> {
        let mut strats = Vec::with_capacity(count);
        for i in 0..count {
            strats.push(arb_block(i as u64, step));
        }
        strategy::Strategy::prop_map(strats, move |strats| strats)
    }

    proptest! {
        #[test]
        fn test_get_insert(blocks in arb_blocklist(10, 1000)) {
            let db = test_utils::create_test_db();
            let tx = db.tx_mut().expect("failed to create rw tx");

            for block in &blocks {
                let mut trie = TrieDbMut::<tables::ClassesTrie, _>::new(&tx);

                // Insert key/value pairs
                for ((r#type, key), value) in &block.keyvalues {
                    let db_key = match r#type {
                        TrieDatabaseKeyType::Trie => DatabaseKey::Trie(key.as_ref()),
                        TrieDatabaseKeyType::Flat => DatabaseKey::Flat(key.as_ref()),
                        TrieDatabaseKeyType::TrieLog => DatabaseKey::TrieLog(key.as_ref()),
                     };

                    trie.insert(&db_key, value.as_ref(), None).expect("failed to insert");
                }

                let snapshot_id = CommitId::from(block.number);
                trie.snapshot(snapshot_id);
            }

            tx.commit().expect("failed to commit tx");
            let tx = db.tx().expect("failed to create ro tx");

            for block in &blocks {
                let snapshot_id = CommitId::from(block.number);
                let snapshot_db = SnapshotTrieDb::<tables::ClassesTrie, _>::new(&tx, snapshot_id);

                // Verify snapshots
                for ((r#type, key), value) in &block.keyvalues {
                    let db_key = match r#type {
                        TrieDatabaseKeyType::Trie => DatabaseKey::Trie(key.as_ref()),
                        TrieDatabaseKeyType::Flat => DatabaseKey::Flat(key.as_ref()),
                        TrieDatabaseKeyType::TrieLog => DatabaseKey::TrieLog(key.as_ref()),
                     };

                    let result = snapshot_db.get(&db_key).unwrap();
                    prop_assert_eq!(result.as_ref().map(|x| x.as_slice()), Some(value.as_slice()));
                }
            }
        }
    }
}
