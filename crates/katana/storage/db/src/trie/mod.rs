use core::fmt;
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;

use anyhow::Result;
use katana_primitives::block::BlockNumber;
use katana_primitives::ContractAddress;
use katana_trie::bonsai::{BonsaiDatabase, BonsaiPersistentDatabase, ByteVec, DatabaseKey};
use katana_trie::CommitId;
use smallvec::ToSmallVec;

use crate::abstraction::{DbCursor, DbTxMutRef, DbTxRef};
use crate::models::trie::{TrieDatabaseKey, TrieDatabaseKeyType, TrieHistoryEntry};
use crate::models::{self};
use crate::tables::{self, Trie};

mod snapshot;

pub use snapshot::SnapshotTrieDb;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] crate::error::DatabaseError);

impl katana_trie::bonsai::DBError for Error {}

#[derive(Debug)]
pub struct TrieDbFactory<'a, Tx: DbTxRef<'a>> {
    tx: Tx,
    _phantom: &'a PhantomData<()>,
}

impl<'a, Tx: DbTxRef<'a>> TrieDbFactory<'a, Tx> {
    pub fn new(tx: Tx) -> Self {
        Self { tx, _phantom: &PhantomData }
    }

    pub fn latest(&self) -> GlobalTrie<'a, Tx> {
        GlobalTrie { tx: self.tx.clone(), _phantom: &PhantomData }
    }

    // TODO: check that the snapshot for the block number is available
    pub fn historical(&self, block: BlockNumber) -> Option<HistoricalGlobalTrie<'a, Tx>> {
        Some(HistoricalGlobalTrie { tx: self.tx.clone(), block, _phantom: &PhantomData })
    }
}

/// Provides access to the latest tries.
#[derive(Debug)]
pub struct GlobalTrie<'a, Tx: DbTxRef<'a>> {
    tx: Tx,
    _phantom: &'a PhantomData<()>,
}

impl<'a, Tx> GlobalTrie<'a, Tx>
where
    Tx: DbTxRef<'a> + Debug,
{
    /// Returns the contracts trie.
    pub fn contracts_trie(
        &self,
    ) -> katana_trie::ContractsTrie<TrieDb<'a, tables::ContractsTrie, Tx>> {
        katana_trie::ContractsTrie::new(TrieDb::new(self.tx.clone()))
    }

    /// Returns the classes trie.
    pub fn classes_trie(&self) -> katana_trie::ClassesTrie<TrieDb<'a, tables::ClassesTrie, Tx>> {
        katana_trie::ClassesTrie::new(TrieDb::new(self.tx.clone()))
    }

    // TODO: makes this return an Option
    /// Returns the storages trie.
    pub fn storages_trie(
        &self,
        address: ContractAddress,
    ) -> katana_trie::StoragesTrie<TrieDb<'a, tables::StoragesTrie, Tx>> {
        katana_trie::StoragesTrie::new(TrieDb::new(self.tx.clone()), address)
    }
}

/// Historical tries, allowing access to the state tries at each block.
#[derive(Debug)]
pub struct HistoricalGlobalTrie<'a, Tx: DbTxRef<'a>> {
    /// The database transaction.
    tx: Tx,
    /// The block number at which the trie was constructed.
    block: BlockNumber,
    _phantom: &'a PhantomData<()>,
}

impl<'a, Tx> HistoricalGlobalTrie<'a, Tx>
where
    Tx: DbTxRef<'a> + Debug,
{
    /// Returns the historical contracts trie.
    pub fn contracts_trie(
        &self,
    ) -> katana_trie::ContractsTrie<SnapshotTrieDb<'a, tables::ContractsTrie, Tx>> {
        let commit = CommitId::new(self.block);
        katana_trie::ContractsTrie::new(SnapshotTrieDb::new(self.tx.clone(), commit))
    }

    /// Returns the historical classes trie.
    pub fn classes_trie(
        &self,
    ) -> katana_trie::ClassesTrie<SnapshotTrieDb<'a, tables::ClassesTrie, Tx>> {
        let commit = CommitId::new(self.block);
        katana_trie::ClassesTrie::new(SnapshotTrieDb::new(self.tx.clone(), commit))
    }

    // TODO: makes this return an Option
    /// Returns the historical storages trie.
    pub fn storages_trie(
        &self,
        address: ContractAddress,
    ) -> katana_trie::StoragesTrie<SnapshotTrieDb<'a, tables::StoragesTrie, Tx>> {
        let commit = CommitId::new(self.block);
        katana_trie::StoragesTrie::new(SnapshotTrieDb::new(self.tx.clone(), commit), address)
    }
}

// --- Trie's database implementations. These are implemented based on the Bonsai Trie
// functionalities and abstractions.

pub struct TrieDb<'a, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'a>,
{
    tx: Tx,
    _phantom: &'a PhantomData<Tb>,
}

impl<'a, Tb, Tx> fmt::Debug for TrieDb<'a, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'a> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrieDbMut").field("tx", &self.tx).finish()
    }
}

impl<'a, Tb, Tx> TrieDb<'a, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'a>,
{
    pub(crate) fn new(tx: Tx) -> Self {
        Self { tx, _phantom: &PhantomData }
    }
}

impl<'a, Tb, Tx> BonsaiDatabase for TrieDb<'a, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxRef<'a> + fmt::Debug,
{
    type Batch = ();
    type DatabaseError = Error;

    fn create_batch(&self) -> Self::Batch {}

    fn remove_by_prefix(&mut self, _: &DatabaseKey<'_>) -> Result<(), Self::DatabaseError> {
        Ok(())
    }

    fn get(&self, key: &DatabaseKey<'_>) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let value = self.tx.get::<Tb>(to_db_key(key))?;
        Ok(value)
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
        unimplemented!("not supported in read-only transaction")
    }

    fn remove(
        &mut self,
        _: &DatabaseKey<'_>,
        _: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        unimplemented!("not supported in read-only transaction")
    }

    fn contains(&self, key: &DatabaseKey<'_>) -> Result<bool, Self::DatabaseError> {
        let key = to_db_key(key);
        let value = self.tx.get::<Tb>(key)?;
        Ok(value.is_some())
    }

    fn write_batch(&mut self, _: Self::Batch) -> Result<(), Self::DatabaseError> {
        unimplemented!("not supported in read-only transaction")
    }
}

pub struct TrieDbMut<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxMutRef<'tx>,
{
    tx: Tx,
    /// List of key-value pairs that has been added throughout the duration of the trie
    /// transaction.
    ///
    /// This will be used to create the trie snapshot.
    write_cache: HashMap<TrieDatabaseKey, ByteVec>,
    _phantom: &'tx PhantomData<Tb>,
}

impl<'tx, Tb, Tx> fmt::Debug for TrieDbMut<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxMutRef<'tx> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrieDbMut").field("tx", &self.tx).finish()
    }
}

impl<'tx, Tb, Tx> TrieDbMut<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxMutRef<'tx>,
{
    pub fn new(tx: Tx) -> Self {
        Self { tx, write_cache: HashMap::new(), _phantom: &PhantomData }
    }
}

impl<'tx, Tb, Tx> BonsaiDatabase for TrieDbMut<'tx, Tb, Tx>
where
    Tb: Trie,
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
        batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let _ = batch;
        let key = to_db_key(key);
        let value: ByteVec = value.to_smallvec();

        let old_value = self.tx.get::<Tb>(key.clone())?;
        self.tx.put::<Tb>(key.clone(), value.clone())?;

        self.write_cache.insert(key, value);
        Ok(old_value)
    }

    fn remove(
        &mut self,
        key: &DatabaseKey<'_>,
        batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        let _ = batch;
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

    fn write_batch(&mut self, _: Self::Batch) -> Result<(), Self::DatabaseError> {
        Ok(())
    }
}

impl<'tx, Tb, Tx> BonsaiPersistentDatabase<CommitId> for TrieDbMut<'tx, Tb, Tx>
where
    Tb: Trie,
    Tx: DbTxMutRef<'tx> + fmt::Debug + 'tx,
{
    type DatabaseError = Error;
    type Transaction<'a> = SnapshotTrieDb<'tx, Tb, Tx>  where Self: 'a;

    fn snapshot(&mut self, id: CommitId) {
        let block_number: BlockNumber = id.into();

        let entries = std::mem::take(&mut self.write_cache);
        let entries = entries.into_iter().map(|(key, value)| TrieHistoryEntry { key, value });

        for entry in entries {
            let mut set = self
                .tx
                .get::<Tb::Changeset>(entry.key.clone())
                .expect("failed to get trie change set")
                .unwrap_or_default();
            set.insert(block_number);

            self.tx
                .put::<Tb::Changeset>(entry.key.clone(), set)
                .expect("failed to put trie change set");

            self.tx
                .put::<Tb::History>(block_number, entry)
                .expect("failed to put trie history entry");
        }
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

#[cfg(test)]
mod tests {
    use katana_primitives::hash::{Poseidon, StarkHash};
    use katana_primitives::{felt, hash};
    use katana_trie::{verify_proof, ClassesTrie, CommitId};
    use starknet::macros::short_string;

    use super::TrieDbMut;
    use crate::abstraction::Database;
    use crate::mdbx::test_utils;
    use crate::tables;
    use crate::trie::SnapshotTrieDb;

    #[test]
    fn snapshot() {
        let db = test_utils::create_test_db();
        let db_tx = db.tx_mut().expect("failed to get tx");

        let mut trie = ClassesTrie::new(TrieDbMut::<tables::ClassesTrie, _>::new(&db_tx));

        let root0 = {
            let entries = [
                (felt!("0x9999"), felt!("0xdead")),
                (felt!("0x5555"), felt!("0xbeef")),
                (felt!("0x1337"), felt!("0xdeadbeef")),
            ];

            for (key, value) in entries {
                trie.insert(key, value);
            }

            trie.commit(0);
            trie.root()
        };

        let root1 = {
            let entries = [
                (felt!("0x6969"), felt!("0x80085")),
                (felt!("0x3333"), felt!("0x420")),
                (felt!("0x2222"), felt!("0x7171")),
            ];

            for (key, value) in entries {
                trie.insert(key, value);
            }

            trie.commit(1);
            trie.root()
        };

        assert_ne!(root0, root1);

        {
            let db = SnapshotTrieDb::<tables::ClassesTrie, _>::new(&db_tx, CommitId::new(0));
            let mut snapshot0 = ClassesTrie::new(db);

            let snapshot_root0 = snapshot0.root();
            assert_eq!(snapshot_root0, root0);

            let proofs0 = snapshot0.multiproof(vec![felt!("0x9999")]);
            let verify_result0 =
                verify_proof::<Poseidon>(&proofs0, snapshot_root0, vec![felt!("0x9999")]);

            let value =
                hash::Poseidon::hash(&short_string!("CONTRACT_CLASS_LEAF_V0"), &felt!("0xdead"));
            assert_eq!(vec![value], verify_result0);
        }

        {
            let commit = CommitId::new(1);
            let mut snapshot1 =
                ClassesTrie::new(SnapshotTrieDb::<tables::ClassesTrie, _>::new(&db_tx, commit));

            let snapshot_root1 = snapshot1.root();
            assert_eq!(snapshot_root1, root1);

            let proofs1 = snapshot1.multiproof(vec![felt!("0x6969")]);
            let verify_result1 =
                verify_proof::<Poseidon>(&proofs1, snapshot_root1, vec![felt!("0x6969")]);

            let value =
                hash::Poseidon::hash(&short_string!("CONTRACT_CLASS_LEAF_V0"), &felt!("0x80085"));
            assert_eq!(vec![value], verify_result1);
        }

        {
            let root = trie.root();
            let proofs = trie.multiproof(vec![felt!("0x6969"), felt!("0x9999")]);
            let result =
                verify_proof::<Poseidon>(&proofs, root, vec![felt!("0x6969"), felt!("0x9999")]);

            let value0 =
                hash::Poseidon::hash(&short_string!("CONTRACT_CLASS_LEAF_V0"), &felt!("0x80085"));
            let value1 =
                hash::Poseidon::hash(&short_string!("CONTRACT_CLASS_LEAF_V0"), &felt!("0xdead"));

            assert_eq!(vec![value0, value1], result);
        }
    }
}
