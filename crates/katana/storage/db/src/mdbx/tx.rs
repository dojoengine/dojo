//! Transaction wrapper for libmdbx-sys.

use std::marker::PhantomData;

use libmdbx::ffi::DBI;
use libmdbx::{TransactionKind, WriteFlags, RW};
use parking_lot::RwLock;

use super::cursor::Cursor;
use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::tables::{Schema, SchemaV1, Table, NUM_TABLES};
use crate::utils::decode_one;

/// Alias for read-only transaction on the default schema.
pub type TxRO = Tx<libmdbx::RO, SchemaV1>;
/// Alias for read-write transaction on the default schema.
pub type TxRW = Tx<libmdbx::RW, SchemaV1>;

/// Database transaction.
///
/// Wrapper for a `libmdbx` transaction.
#[derive(Debug)]
pub struct Tx<K: TransactionKind, S: Schema> {
    /// Libmdbx-sys transaction.
    inner: libmdbx::Transaction<K>,
    /// Marker for the db schema.
    _schema: std::marker::PhantomData<S>,
    // the array size is hardcoded to the number of tables in current db version for now. ideally
    // we could use the associated constant from the schema trait. but that would require the
    // `generic_const_exprs`.
    /// Database table handle cache.
    db_handles: RwLock<[Option<DBI>; NUM_TABLES]>,
}

impl<K, S> Tx<K, S>
where
    K: TransactionKind,
    S: Schema,
{
    /// Creates new `Tx` object with a `RO` or `RW` transaction.
    pub fn new(inner: libmdbx::Transaction<K>) -> Self {
        Self { inner, _schema: PhantomData, db_handles: Default::default() }
    }

    /// Creates a cursor to iterate over a table items.
    pub fn cursor<T: Table>(&self) -> Result<Cursor<K, T>, DatabaseError> {
        self.inner
            .cursor_with_dbi(self.get_dbi::<T>()?)
            .map(Cursor::new)
            .map_err(DatabaseError::CreateCursor)
    }

    /// Gets a table database handle if it exists, otherwise creates it.
    pub fn get_dbi<T: Table>(&self) -> Result<DBI, DatabaseError> {
        // SAFETY:
        // the index is guaranteed to be in bounds by the schema only on current schema
        // version because we hardcode the size exactly for the number of tables in current db
        // schema. see `tables::v1::NUM_TABLES`.
        let table = S::index::<T>().expect(&format!("table {} not found in schema", T::NAME));

        let mut handles = self.db_handles.write();
        let dbi_handle = handles.get_mut(table).expect("should exist");

        if dbi_handle.is_none() {
            let dbi = self.inner.open_db(Some(T::NAME)).map_err(DatabaseError::OpenDb)?.dbi();
            *dbi_handle = Some(dbi);
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

    /// Gets a value from a table using the given key without checking if the table exist in the
    /// schema.
    pub fn get_unchecked<T: Table>(
        &self,
        key: T::Key,
    ) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        let dbi = self.inner.open_db(Some(T::NAME)).map_err(DatabaseError::OpenDb)?.dbi();
        let key = Encode::encode(key);

        self.inner
            .get(dbi, key.as_ref())
            .map_err(DatabaseError::Read)?
            .map(decode_one::<T>)
            .transpose()
    }

    /// Returns number of entries in the table using cheap DB stats invocation.
    pub fn entries<T: Table>(&self) -> Result<usize, DatabaseError> {
        self.inner
            .db_stat_with_dbi(self.get_dbi::<T>()?)
            .map(|stat| stat.entries())
            .map_err(DatabaseError::Stat)
    }

    /// Commits the transaction.
    pub fn commit(self) -> Result<bool, DatabaseError> {
        self.inner.commit().map_err(DatabaseError::Commit)
    }
}

impl<S: Schema> Tx<RW, S> {
    /// Inserts an item into a database.
    ///
    /// This function stores key/data pairs in the database. The default behavior is to enter the
    /// new key/data pair, replacing any previously existing key if duplicates are disallowed, or
    /// adding a duplicate data item if duplicates are allowed (DatabaseFlags::DUP_SORT).
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

    /// Clears all entries in the given database. This will emtpy the database.
    pub fn clear<T: Table>(&self) -> Result<(), DatabaseError> {
        self.inner.clear_db(self.get_dbi::<T>()?).map_err(DatabaseError::Clear)
    }

    /// Aborts the transaction.
    pub fn abort(self) {
        drop(self.inner)
    }
}
