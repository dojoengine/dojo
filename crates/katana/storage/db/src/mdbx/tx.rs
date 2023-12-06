//! Transaction wrapper for libmdbx-sys.

use std::str::FromStr;

use libmdbx::ffi::DBI;
use libmdbx::{TransactionKind, WriteFlags, RW};
use parking_lot::RwLock;

use super::cursor::Cursor;
use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::tables::{DupSort, Table, Tables, NUM_TABLES};
use crate::utils::decode_one;

/// Database transaction.
///
/// Wrapper for a `libmdbx` transaction.
#[derive(Debug)]
pub struct Tx<K: TransactionKind> {
    /// Libmdbx-sys transaction.
    pub inner: libmdbx::Transaction<K>,
    /// Database table handle cache.
    pub(crate) db_handles: RwLock<[Option<DBI>; NUM_TABLES]>,
}

impl<K: TransactionKind> Tx<K> {
    /// Creates new `Tx` object with a `RO` or `RW` transaction.
    pub fn new(inner: libmdbx::Transaction<K>) -> Self {
        Self { inner, db_handles: Default::default() }
    }

    /// Create db Cursor
    pub fn new_cursor<T: Table>(&self) -> Result<Cursor<K, T>, DatabaseError> {
        let inner = self
            .inner
            .cursor_with_dbi(self.get_dbi::<T>()?)
            .map_err(DatabaseError::CreateCursor)?;

        Ok(Cursor::new(inner))
    }

    /// Gets a table database handle if it exists, otherwise creates it.
    pub fn get_dbi<T: Table>(&self) -> Result<DBI, DatabaseError> {
        let mut handles = self.db_handles.write();
        let table = Tables::from_str(T::NAME).expect("Requested table should be part of `Tables`.");

        let dbi_handle = handles.get_mut(table as usize).expect("should exist");
        if dbi_handle.is_none() {
            *dbi_handle =
                Some(self.inner.open_db(Some(T::NAME)).map_err(DatabaseError::OpenDb)?.dbi());
        }

        Ok(dbi_handle.expect("is some; qed"))
    }

    /// Gets a value from a table using the given key.
    pub fn get<T: Table>(&self, key: T::Key) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        let key = Encode::encode(key);
        self.inner
            .get(self.get_dbi::<T>()?, key.as_ref())
            .map_err(DatabaseError::Read)?
            .map(decode_one::<T>)
            .transpose()
    }

    /// Returns number of entries in the table using cheap DB stats invocation.
    pub fn entries<T: Table>(&self) -> Result<usize, DatabaseError> {
        Ok(self
            .inner
            .db_stat_with_dbi(self.get_dbi::<T>()?)
            .map_err(DatabaseError::Stat)?
            .entries())
    }

    // Creates a cursor to iterate over a table values.
    pub fn cursor<T: Table>(&self) -> Result<Cursor<K, T>, DatabaseError> {
        self.new_cursor()
    }

    // Creates a cursor to iterate over a `DUPSORT` table values.
    pub fn cursor_dup<T: DupSort>(&self) -> Result<Cursor<K, T>, DatabaseError> {
        self.new_cursor()
    }

    /// Commits the transaction.
    pub fn commit(self) -> Result<bool, DatabaseError> {
        self.inner.commit().map_err(DatabaseError::Commit)
    }
}

impl Tx<RW> {
    pub fn put<T: Table>(&self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = key.encode();
        let value = value.compress();
        self.inner.put(self.get_dbi::<T>()?, key, value, WriteFlags::UPSERT).unwrap();
        Ok(())
    }

    /// Delete items from a database, removing the key/data pair if it exists.
    ///
    /// If the data parameter is [Some] only the matching data item will be deleted. Otherwise, if
    /// data parameter is [None], any/all value(s) for specified key will be deleted.
    ///
    /// Returns `true` if the key/value pair was present.
    pub fn delete<T: Table>(
        &self,
        key: T::Key,
        value: Option<T::Value>,
    ) -> Result<bool, DatabaseError> {
        let value = value.map(Compress::compress);
        let value = value.as_ref().map(|v| v.as_ref());
        self.inner.del(self.get_dbi::<T>()?, key.encode(), value).map_err(DatabaseError::Delete)
    }

    pub fn clear<T: Table>(&self) -> Result<(), DatabaseError> {
        self.inner.clear_db(self.get_dbi::<T>()?).map_err(DatabaseError::Clear)
    }

    /// Aborts the transaction.
    pub fn abort(self) {
        drop(self.inner)
    }
}

impl<K> From<libmdbx::Transaction<K>> for Tx<K>
where
    K: TransactionKind,
{
    fn from(inner: libmdbx::Transaction<K>) -> Self {
        Tx::new(inner)
    }
}
