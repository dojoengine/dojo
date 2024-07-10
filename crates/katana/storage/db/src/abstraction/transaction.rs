use super::cursor::{DbCursor, DbCursorMut};
use crate::error::DatabaseError;
use crate::tables::Table;

/// Trait for read-only transaction type.
pub trait DbTx {
    /// The cursor type.
    type Cursor<T: Table>: DbCursor<T>;

    /// Creates a cursor to iterate over a table items.
    fn cursor<T: Table>(&self) -> Result<Self::Cursor<T>, DatabaseError>;

    /// Gets a value from a table using the given key.
    fn get<T: Table>(&self, key: T::Key) -> Result<Option<T::Value>, DatabaseError>;

    /// Returns number of entries in the table.
    fn entries<T: Table>(&self) -> Result<usize, DatabaseError>;

    /// Commits the transaction.
    fn commit(self) -> Result<bool, DatabaseError>;

    /// Aborts the transaction.
    fn abort(self);
}

/// Trait for read-write transaction type.
pub trait DbTxMut: DbTx {
    /// The mutable cursor type.
    type Cursor<T: Table>: DbCursorMut<T>;

    /// Creates a cursor to mutably iterate over a table items.
    fn cursor_mut<T: Table>(&self) -> Result<<Self as DbTxMut>::Cursor<T>, DatabaseError>;

    /// Inserts an item into a database.
    ///
    /// This function stores key/data pairs in the database. The default behavior is to enter the
    /// new key/data pair, replacing any previously existing key if duplicates are disallowed, or
    /// adding a duplicate data item if duplicates are allowed (DatabaseFlags::DUP_SORT).
    fn put<T: Table>(&self, key: T::Key, value: T::Value) -> Result<(), DatabaseError>;

    /// Delete items from a database, removing the key/data pair if it exists.
    ///
    /// If the data parameter is [Some] only the matching data item will be deleted. Otherwise, if
    /// data parameter is [None], any/all value(s) for specified key will be deleted.
    ///
    /// Returns `true` if the key/value pair was present.
    fn delete<T: Table>(&self, key: T::Key, value: Option<T::Value>)
    -> Result<bool, DatabaseError>;

    /// Clears all entries in the given database. This will empty the database.
    fn clear<T: Table>(&self) -> Result<(), DatabaseError>;
}
