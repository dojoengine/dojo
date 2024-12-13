use super::cursor::{DbCursor, DbCursorMut};
use super::{DbDupSortCursor, DbDupSortCursorMut};
use crate::error::DatabaseError;
use crate::tables::{DupSort, Table};

/// Trait for read-only transaction type.
pub trait DbTx {
    /// The cursor type.
    type Cursor<T: Table>: DbCursor<T>;

    /// The cursor type for dupsort table.
    // TODO: ideally we should only define the cursor type once,
    // find a way to not have to define both cursor types in both traits
    type DupCursor<T: DupSort>: DbDupSortCursor<T>;

    /// Creates a cursor to iterate over a table items.
    fn cursor<T: Table>(&self) -> Result<Self::Cursor<T>, DatabaseError>;

    /// Creates a cursor to iterate over a dupsort table items.
    fn cursor_dup<T: DupSort>(&self) -> Result<Self::DupCursor<T>, DatabaseError>;

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

    /// The mutable cursor type for dupsort table.
    // TODO: find a way to not have to define both cursor types in both traits
    type DupCursor<T: DupSort>: DbDupSortCursorMut<T>;

    /// Creates a cursor to mutably iterate over a table items.
    fn cursor_mut<T: Table>(&self) -> Result<<Self as DbTxMut>::Cursor<T>, DatabaseError>;

    /// Creates a cursor to iterate over a dupsort table items.
    fn cursor_dup_mut<T: DupSort>(&self) -> Result<<Self as DbTxMut>::DupCursor<T>, DatabaseError>;

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

// --- Reference variations

pub trait DbTxRef<'a>: Clone {
    /// The cursor type.
    type Cursor<T: Table>: DbCursor<T>;

    /// The cursor type for dupsort table.
    // TODO: ideally we should only define the cursor type once,
    // find a way to not have to define both cursor types in both traits
    type DupCursor<T: DupSort>: DbDupSortCursor<T>;

    /// Creates a cursor to iterate over a table items.
    fn cursor<T: Table>(&self) -> Result<Self::Cursor<T>, DatabaseError>;

    /// Creates a cursor to iterate over a dupsort table items.
    fn cursor_dup<T: DupSort>(&self) -> Result<Self::DupCursor<T>, DatabaseError>;

    /// Gets a value from a table using the given key.
    fn get<T: Table>(&self, key: T::Key) -> Result<Option<T::Value>, DatabaseError>;

    /// Returns number of entries in the table.
    fn entries<T: Table>(&self) -> Result<usize, DatabaseError>;
}

pub trait DbTxMutRef<'a>: DbTxRef<'a> {
    /// The mutable cursor type.
    type Cursor<T: Table>: DbCursorMut<T>;

    /// The mutable cursor type for dupsort table.
    // TODO: find a way to not have to define both cursor types in both traits
    type DupCursor<T: DupSort>: DbDupSortCursorMut<T>;

    /// Creates a cursor to mutably iterate over a table items.
    fn cursor_mut<T: Table>(&self) -> Result<<Self as DbTxMutRef<'a>>::Cursor<T>, DatabaseError>;

    /// Creates a cursor to iterate over a dupsort table items.
    fn cursor_dup_mut<T: DupSort>(
        &self,
    ) -> Result<<Self as DbTxMutRef<'a>>::DupCursor<T>, DatabaseError>;

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

impl<'a, Tx> DbTxRef<'a> for &'a Tx
where
    Tx: DbTx,
{
    type Cursor<T: Table> = <Tx as DbTx>::Cursor<T>;
    type DupCursor<T: DupSort> = <Tx as DbTx>::DupCursor<T>;

    fn cursor<T: Table>(&self) -> Result<Self::Cursor<T>, DatabaseError> {
        <Tx as DbTx>::cursor::<T>(self)
    }

    fn cursor_dup<T: DupSort>(&self) -> Result<Self::DupCursor<T>, DatabaseError> {
        <Tx as DbTx>::cursor_dup::<T>(self)
    }

    fn entries<T: Table>(&self) -> Result<usize, DatabaseError> {
        <Tx as DbTx>::entries::<T>(self)
    }

    fn get<T: Table>(&self, key: T::Key) -> Result<Option<T::Value>, DatabaseError> {
        <Tx as DbTx>::get::<T>(self, key)
    }
}

impl<'a, Tx> DbTxMutRef<'a> for &'a Tx
where
    Tx: DbTxMut,
{
    type Cursor<T: Table> = <Tx as DbTxMut>::Cursor<T>;
    type DupCursor<T: DupSort> = <Tx as DbTxMut>::DupCursor<T>;

    fn cursor_mut<T: Table>(&self) -> Result<<Self as DbTxMutRef<'a>>::Cursor<T>, DatabaseError> {
        <Tx as DbTxMut>::cursor_mut::<T>(self)
    }

    fn cursor_dup_mut<T: DupSort>(
        &self,
    ) -> Result<<Self as DbTxMutRef<'a>>::DupCursor<T>, DatabaseError> {
        <Tx as DbTxMut>::cursor_dup_mut::<T>(self)
    }

    fn put<T: Table>(&self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        <Tx as DbTxMut>::put::<T>(self, key, value)
    }

    fn delete<T: Table>(
        &self,
        key: T::Key,
        value: Option<T::Value>,
    ) -> Result<bool, DatabaseError> {
        <Tx as DbTxMut>::delete::<T>(self, key, value)
    }

    fn clear<T: Table>(&self) -> Result<(), DatabaseError> {
        <Tx as DbTxMut>::clear::<T>(self)
    }
}
