//! Transaction wrapper for libmdbx-sys.

use std::str::FromStr;
use std::sync::Arc;

use libmdbx::ffi::DBI;
use libmdbx::{EnvironmentKind, Transaction, TransactionKind, WriteFlags, RW};
use parking_lot::RwLock;

use super::cursor::Cursor;
use super::tables::{DupSort, Table, Tables, NUM_TABLES};
use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::utils::decode_one;

/// Wrapper for a `libmdbx` transaction.
#[derive(Debug)]
pub struct Tx<'env, K: TransactionKind, E: EnvironmentKind> {
    /// Libmdbx-sys transaction.
    pub inner: libmdbx::Transaction<'env, K, E>,
    /// Database table handle cache.
    pub(crate) db_handles: Arc<RwLock<[Option<DBI>; NUM_TABLES]>>,
}

impl<K: TransactionKind, E: EnvironmentKind> Tx<'_, K, E> {
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

    fn get<T: Table>(&self, key: T::Key) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        self.inner
            .get(self.get_dbi::<T>()?, key.encode().as_ref())
            .map_err(DatabaseError::Read)?
            .map(decode_one::<T>)
            .transpose()
    }

    /// Commits the transaction.
    fn commit(self) -> Result<bool, DatabaseError> {
        self.inner.commit().map_err(DatabaseError::Commit)
    }

    /// Aborts the transaction.
    fn abort(self) {
        drop(self.inner)
    }

    /// Returns number of entries in the table using cheap DB stats invocation.
    fn entries<T: Table>(&self) -> Result<usize, DatabaseError> {
        Ok(self
            .inner
            .db_stat_with_dbi(self.get_dbi::<T>()?)
            .map_err(DatabaseError::Stat)?
            .entries())
    }
}

impl<'env, K: TransactionKind, E: EnvironmentKind> Tx<'env, K, E> {
    /// Creates new `Tx` object with a `RO` or `RW` transaction.
    pub fn new(inner: Transaction<'env, K, E>) -> Self {
        Self { inner, db_handles: Default::default() }
    }

    /// Create db Cursor
    pub fn new_cursor<T: Table>(&self) -> Result<Cursor<'env, K, T>, DatabaseError> {
        let inner = self
            .inner
            .cursor_with_dbi(self.get_dbi::<T>()?)
            .map_err(DatabaseError::CreateCursor)?;

        Ok(Cursor::new(inner))
    }

    // Iterate over read only values in database.
    fn cursor_read<T: Table>(&self) -> Result<Cursor<'env, K, T>, DatabaseError> {
        self.new_cursor()
    }

    /// Iterate over read only values in database.
    fn cursor_dup_read<T: DupSort>(&self) -> Result<Cursor<'env, K, T>, DatabaseError> {
        self.new_cursor()
    }
}

impl<E: EnvironmentKind> Tx<'_, RW, E> {
    fn put<T: Table>(&self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = key.encode();
        let value = value.compress();
        self.inner.put(self.get_dbi::<T>()?, key, value, WriteFlags::UPSERT).unwrap();
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

impl<'env, E: EnvironmentKind> Tx<'env, RW, E> {
    fn cursor_write<T: Table>(&self) -> Result<Cursor<'env, RW, T>, DatabaseError> {
        self.new_cursor()
    }

    fn cursor_dup_write<T: DupSort>(&self) -> Result<Cursor<'env, RW, T>, DatabaseError> {
        self.new_cursor()
    }
}
