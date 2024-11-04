//! Transaction wrapper for libmdbx-sys.

use std::str::FromStr;

use libmdbx::ffi::DBI;
use libmdbx::{TransactionKind, WriteFlags, RW};
use parking_lot::RwLock;

use super::cursor::Cursor;
use super::stats::TableStat;
use crate::abstraction::{DbTx, DbTxMut};
use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::tables::{DupSort, Table, Tables, NUM_TABLES};
use crate::utils::decode_one;

/// Alias for read-only transaction.
pub type TxRO = Tx<libmdbx::RO>;
/// Alias for read-write transaction.
pub type TxRW = Tx<libmdbx::RW>;

/// Database transaction.
///
/// Wrapper for a `libmdbx` transaction.
#[derive(Debug)]
pub struct Tx<K: TransactionKind> {
    /// Libmdbx-sys transaction.
    pub(super) inner: libmdbx::Transaction<K>,
    /// Database table handle cache.
    db_handles: RwLock<[Option<DBI>; NUM_TABLES]>,
}

impl<K: TransactionKind> Tx<K> {
    /// Creates new `Tx` object with a `RO` or `RW` transaction.
    pub fn new(inner: libmdbx::Transaction<K>) -> Self {
        Self { inner, db_handles: Default::default() }
    }

    pub fn get_dbi<T: Table>(&self) -> Result<DBI, DatabaseError> {
        let mut handles = self.db_handles.write();
        let table = Tables::from_str(T::NAME).expect("requested table should be part of `Tables`.");

        let dbi_handle = handles.get_mut(table as usize).expect("should exist");
        if dbi_handle.is_none() {
            *dbi_handle =
                Some(self.inner.open_db(Some(T::NAME)).map_err(DatabaseError::OpenDb)?.dbi());
        }

        Ok(dbi_handle.expect("is some; qed"))
    }

    /// Retrieves statistics for a specific table.
    pub fn stat<T: Table>(&self) -> Result<TableStat, DatabaseError> {
        let dbi = self.get_dbi::<T>()?;
        let stat = self.inner.db_stat_with_dbi(dbi).map_err(DatabaseError::Stat)?;
        Ok(TableStat::new(stat))
    }
}

impl<K: TransactionKind> DbTx for Tx<K> {
    type Cursor<T: Table> = Cursor<K, T>;
    type DupCursor<T: DupSort> = Self::Cursor<T>;

    fn cursor<T: Table>(&self) -> Result<Cursor<K, T>, DatabaseError> {
        self.inner
            .cursor_with_dbi(self.get_dbi::<T>()?)
            .map(Cursor::new)
            .map_err(DatabaseError::CreateCursor)
    }

    fn cursor_dup<T: DupSort>(&self) -> Result<Cursor<K, T>, DatabaseError> {
        self.inner
            .cursor_with_dbi(self.get_dbi::<T>()?)
            .map(Cursor::new)
            .map_err(DatabaseError::CreateCursor)
    }

    fn get<T: Table>(&self, key: T::Key) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        let key = Encode::encode(key);
        self.inner
            .get(self.get_dbi::<T>()?, key.as_ref())
            .map_err(DatabaseError::Read)?
            .map(decode_one::<T>)
            .transpose()
    }

    fn entries<T: Table>(&self) -> Result<usize, DatabaseError> {
        self.inner
            .db_stat_with_dbi(self.get_dbi::<T>()?)
            .map(|stat| stat.entries())
            .map_err(DatabaseError::Stat)
    }

    fn commit(self) -> Result<bool, DatabaseError> {
        self.inner.commit().map_err(DatabaseError::Commit)
    }

    fn abort(self) {
        drop(self.inner)
    }
}

impl DbTxMut for Tx<RW> {
    type Cursor<T: Table> = Cursor<RW, T>;
    type DupCursor<T: DupSort> = <Self as DbTxMut>::Cursor<T>;

    fn cursor_mut<T: Table>(&self) -> Result<<Self as DbTxMut>::Cursor<T>, DatabaseError> {
        DbTx::cursor(self)
    }

    fn cursor_dup_mut<T: DupSort>(&self) -> Result<<Self as DbTxMut>::DupCursor<T>, DatabaseError> {
        self.inner
            .cursor_with_dbi(self.get_dbi::<T>()?)
            .map(Cursor::new)
            .map_err(DatabaseError::CreateCursor)
    }

    fn put<T: Table>(&self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = key.encode();
        let value = value.compress();
        self.inner.put(self.get_dbi::<T>()?, &key, value, WriteFlags::UPSERT).map_err(|error| {
            DatabaseError::Write { error, table: T::NAME, key: Box::from(key.as_ref()) }
        })?;
        Ok(())
    }

    fn delete<T: Table>(
        &self,
        key: T::Key,
        value: Option<T::Value>,
    ) -> Result<bool, DatabaseError> {
        let value = value.map(Compress::compress);
        let value = value.as_ref().map(|v| v.as_ref());
        self.inner.del(self.get_dbi::<T>()?, key.encode(), value).map_err(DatabaseError::Delete)
    }

    fn clear<T: Table>(&self) -> Result<(), DatabaseError> {
        self.inner.clear_db(self.get_dbi::<T>()?).map_err(DatabaseError::Clear)
    }
}
