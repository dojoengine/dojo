use crate::error::DatabaseError;
use crate::tables::{self, DupSort, Table};
use crate::utils::KeyValue;

/// Cursor trait for navigating the items within a database.
pub trait DbCursor<T: Table>: Sized {
    /// Retrieves the first key/value pair, positioning the cursor at the first key/value pair in
    /// the table.
    fn first(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Retrieves key/value pair at current cursor position.
    fn current(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Retrieves the next key/value pair, positioning the cursor at the next key/value pair in
    /// the table.
    fn next(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Retrieves the previous key/value pair, positioning the cursor at the previous key/value pair
    /// in the table.
    fn prev(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Retrieves the last key/value pair, positioning the cursor at the last key/value pair in
    /// the table.
    fn last(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Set the cursor to the specified key, returning and positioning the cursor at the item if
    /// found.
    fn set(&mut self, key: T::Key) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Search for a `key` in a table, returning and positioning the cursor at the first item whose
    /// key is greater than or equal to `key`.
    fn seek(&mut self, key: T::Key) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Creates a walker to iterate over the table items.
    ///
    /// If `start_key` is `None`, the walker will start at the first item of the table. Otherwise,
    /// it will start at the first item whose key is greater than or equal to `start_key`.
    fn walk(&mut self, start_key: Option<T::Key>) -> Result<Walker<'_, T, Self>, DatabaseError>;
}

/// Cursor trait for read-write operations.
pub trait DbCursorMut<T: Table>: DbCursor<T> {
    /// Database operation that will update an existing row if a specified value already
    /// exists in a table, and insert a new row if the specified value doesn't already exist
    fn upsert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError>;

    /// Puts a key/value pair into the database. The cursor will be positioned at the new data item,
    /// or on failure, usually near it.
    fn insert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError>;

    /// Appends the data to the end of the table. Consequently, the append operation
    /// will fail if the inserted key is less than the last table key
    fn append(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError>;

    /// Deletes the current key/value pair.
    fn delete_current(&mut self) -> Result<(), DatabaseError>;
}

/// Cursor trait for DUPSORT tables.
pub trait DbDupSortCursor<T: DupSort>: DbCursor<T> {
    /// Positions the cursor at next data item of current key, returning the next `key-value`
    /// pair of a DUPSORT table.
    fn next_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Similar to `next_dup()`, but instead of returning a `key-value` pair, it returns
    /// only the `value`.
    fn next_dup_val(&mut self) -> Result<Option<T::Value>, DatabaseError>;

    /// Returns the next key/value pair skipping the duplicates.
    fn next_no_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError>;

    /// Search for a `key` and `subkey` pair in a DUPSORT table. Positioning the cursor at the first
    /// item whose `subkey` is greater than or equal to the specified `subkey`.
    fn seek_by_key_subkey(
        &mut self,
        key: <T as Table>::Key,
        subkey: <T as DupSort>::SubKey,
    ) -> Result<Option<<T as Table>::Value>, DatabaseError>;

    /// Depending on its arguments, returns an iterator starting at:
    /// - Some(key), Some(subkey): a `key` item whose data is >= than `subkey`
    /// - Some(key), None: first item of a specified `key`
    /// - None, Some(subkey): like first case, but in the first key
    /// - None, None: first item in the table
    /// of a DUPSORT table.
    fn walk_dup(
        &mut self,
        key: Option<T::Key>,
        subkey: Option<<T as DupSort>::SubKey>,
    ) -> Result<Option<DupWalker<'_, T, Self>>, DatabaseError>;
}

/// Cursor trait for read-write operations on DUPSORT tables.
pub trait DbDupSortCursorMut<T: tables::DupSort>: DbDupSortCursor<T> + DbCursorMut<T> {
    /// Deletes all values for the current key.
    fn delete_current_duplicates(&mut self) -> Result<(), DatabaseError>;

    /// Appends the data as a duplicate for the current key.
    fn append_dup(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError>;
}

/// A key-value pair coming from an iterator.
///
/// The `Result` represents that the operation might fail, while the `Option` represents whether or
/// not there is an entry.
pub type IterPairResult<T> = Option<Result<KeyValue<T>, DatabaseError>>;

/// Provides an iterator to a `Cursor` when handling `Table`.
#[derive(Debug)]
pub struct Walker<'c, T: Table, C: DbCursor<T>> {
    /// Cursor to be used to walk through the table.
    cursor: &'c mut C,
    /// Initial position of the dup walker. The value (key/value pair)  where to start the walk.
    start: IterPairResult<T>,
}

impl<'c, T, C> Walker<'c, T, C>
where
    T: Table,
    C: DbCursor<T>,
{
    /// Create a new [`Walker`] from a [`Cursor`] and a [`IterPairResult`].
    pub fn new(cursor: &'c mut C, start: IterPairResult<T>) -> Self {
        Self { cursor, start }
    }
}

impl<T, C> Walker<'_, T, C>
where
    T: Table,
    C: DbCursorMut<T>,
{
    /// Delete the `key/value` pair item at the current position of the walker.
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.cursor.delete_current()
    }
}

impl<T, C> std::iter::Iterator for Walker<'_, T, C>
where
    T: Table,
    C: DbCursor<T>,
{
    type Item = Result<KeyValue<T>, DatabaseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if let value @ Some(_) = self.start.take() { value } else { self.cursor.next().transpose() }
    }
}

/// A cursor iterator for `DUPSORT` table.
///
/// Similar to [`Walker`], but for `DUPSORT` table.
#[derive(Debug)]
pub struct DupWalker<'c, T: Table, C: DbCursor<T>> {
    /// Cursor to be used to walk through the table.
    cursor: &'c mut C,
    /// Initial position of the dup walker. The value (key/value pair) where to start the walk.
    start: IterPairResult<T>,
}

impl<'c, T, C> DupWalker<'c, T, C>
where
    T: DupSort,
    C: DbCursor<T>,
{
    /// Creates a new [`DupWalker`] from a [`Cursor`] and a [`IterPairResult`].
    pub fn new(cursor: &'c mut C, start: IterPairResult<T>) -> Self {
        Self { cursor, start }
    }
}

impl<T, C> DupWalker<'_, T, C>
where
    T: DupSort,
    C: DbCursorMut<T>,
{
    /// Delete the item at the current position of the walker.
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.cursor.delete_current()
    }
}

impl<T, C> std::iter::Iterator for DupWalker<'_, T, C>
where
    T: DupSort,
    C: DbDupSortCursor<T>,
{
    type Item = Result<KeyValue<T>, DatabaseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if let value @ Some(_) = self.start.take() {
            value
        } else {
            self.cursor.next_dup().transpose()
        }
    }
}
