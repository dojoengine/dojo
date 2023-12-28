//! Cursor wrapper for libmdbx-sys.

use std::borrow::Cow;
use std::marker::PhantomData;

use libmdbx::{self, TransactionKind, WriteFlags, RW};

use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::tables::{DupSort, Table};
use crate::utils::{decode_one, decode_value, decoder, KeyValue};

/// Takes key/value pair from the database and decodes it appropriately.
macro_rules! decode {
    ($v:expr) => {
        $v.map_err($crate::error::DatabaseError::Read)?.map($crate::utils::decoder::<T>).transpose()
    };
}

/// Cursor for navigating the items within a database.
#[derive(Debug)]
pub struct Cursor<K: TransactionKind, T: Table> {
    /// Inner `libmdbx` cursor.
    inner: libmdbx::Cursor<K>,
    /// Phantom data to enforce encoding/decoding.
    _dbi: PhantomData<T>,
}

impl<K: TransactionKind, T: Table> Cursor<K, T> {
    pub(crate) fn new(inner: libmdbx::Cursor<K>) -> Self {
        Self { inner, _dbi: PhantomData }
    }
}

impl<K: TransactionKind, T: Table> Cursor<K, T> {
    /// Retrieves the first key/value pair, positioning the cursor at the first key/value pair in
    /// the table.
    pub fn first(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::first(&mut self.inner))
    }

    /// Retrieves key/value pair at current cursor position.
    pub fn current(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::get_current(&mut self.inner))
    }

    /// Retrieves the next key/value pair, positioning the cursor at the next key/value pair in
    /// the table.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next(&mut self.inner))
    }

    /// Retrieves the previous key/value pair, positioning the cursor at the previous key/value pair
    /// in the table.
    pub fn prev(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::prev(&mut self.inner))
    }

    /// Retrieves the last key/value pair, positioning the cursor at the last key/value pair in
    /// the table.
    pub fn last(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::last(&mut self.inner))
    }

    /// Set the cursor to the specified key, returning and positioning the cursor at the item if
    /// found.
    pub fn set(&mut self, key: <T as Table>::Key) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::set_key(&mut self.inner, key.encode().as_ref()))
    }

    /// Search for a `key` in a table, returning and positioning the cursor at the first item whose
    /// key is greater than or equal to `key`.
    pub fn seek(&mut self, key: <T as Table>::Key) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::set_range(&mut self.inner, key.encode().as_ref()))
    }

    /// Creates a walker to iterate over the table items.
    ///
    /// If `start_key` is `None`, the walker will start at the first item of the table. Otherwise,
    /// it will start at the first item whose key is greater than or equal to `start_key`.
    pub fn walk(&mut self, start_key: Option<T::Key>) -> Result<Walker<'_, K, T>, DatabaseError> {
        let start = if let Some(start_key) = start_key {
            self.inner
                .set_range(start_key.encode().as_ref())
                .map_err(DatabaseError::Read)?
                .map(decoder::<T>)
        } else {
            self.first().transpose()
        };

        Ok(Walker::new(self, start))
    }
}

impl<K: TransactionKind, T: DupSort> Cursor<K, T> {
    /// Positions the cursor at next data item of current key, returning the next `key-value`
    /// pair of a DUPSORT table.
    pub fn next_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_dup(&mut self.inner))
    }

    /// Similar to [`Self::next_dup()`], but instead of returning a `key-value` pair, it returns
    /// only the `value`.
    pub fn next_dup_val(&mut self) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        libmdbx::Cursor::next_dup(&mut self.inner)
            .map_err(DatabaseError::Read)?
            .map(decode_value::<T>)
            .transpose()
    }

    /// Returns the next key/value pair skipping the duplicates.
    pub fn next_no_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_nodup(&mut self.inner))
    }

    /// Search for a `key` and `subkey` pair in a DUPSORT table. Positioning the cursor at the first
    /// item whose `subkey` is greater than or equal to the specified `subkey`.
    pub fn seek_by_key_subkey(
        &mut self,
        key: <T as Table>::Key,
        subkey: <T as DupSort>::SubKey,
    ) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        libmdbx::Cursor::get_both_range(
            &mut self.inner,
            key.encode().as_ref(),
            subkey.encode().as_ref(),
        )
        .map_err(DatabaseError::Read)?
        .map(decode_one::<T>)
        .transpose()
    }

    /// Depending on its arguments, returns an iterator starting at:
    /// - Some(key), Some(subkey): a `key` item whose data is >= than `subkey`
    /// - Some(key), None: first item of a specified `key`
    /// - None, Some(subkey): like first case, but in the first key
    /// - None, None: first item in the table
    /// of a DUPSORT table.
    pub fn walk_dup(
        &mut self,
        key: Option<T::Key>,
        subkey: Option<T::SubKey>,
    ) -> Result<Option<DupWalker<'_, K, T>>, DatabaseError> {
        let start = match (key, subkey) {
            (Some(key), Some(subkey)) => {
                // encode key and decode it after.
                let key: Vec<u8> = key.encode().into();
                self.inner
                    .get_both_range(key.as_ref(), subkey.encode().as_ref())
                    .map_err(DatabaseError::Read)?
                    .map(|val| decoder::<T>((Cow::Owned(key), val)))
            }

            (Some(key), None) => {
                let key: Vec<u8> = key.encode().into();

                let Some(start) = self
                    .inner
                    .set(key.as_ref())
                    .map_err(DatabaseError::Read)?
                    .map(|val| decoder::<T>((Cow::Owned(key), val)))
                else {
                    return Ok(None);
                };

                Some(start)
            }

            (None, Some(subkey)) => {
                if let Some((key, _)) = self.first()? {
                    let key: Vec<u8> = key.encode().into();
                    self.inner
                        .get_both_range(key.as_ref(), subkey.encode().as_ref())
                        .map_err(DatabaseError::Read)?
                        .map(|val| decoder::<T>((Cow::Owned(key), val)))
                } else {
                    Some(Err(DatabaseError::Read(libmdbx::Error::NotFound)))
                }
            }

            (None, None) => self.first().transpose(),
        };

        Ok(Some(DupWalker::new(self, start)))
    }
}

impl<T: Table> Cursor<RW, T> {
    /// Database operation that will update an existing row if a specified value already
    /// exists in a table, and insert a new row if the specified value doesn't already exist
    ///
    /// For a `DUPSORT` table, `upsert` will not actually update-or-insert. If the key already
    /// exists, it will append the value to the subkey, even if the subkeys are the same. So if
    /// you want to properly upsert, you'll need to `seek_exact` & `delete_current` if the
    /// key+subkey was found, before calling `upsert`.
    pub fn upsert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);

        libmdbx::Cursor::put(&mut self.inner, key.as_ref(), value.as_ref(), WriteFlags::UPSERT)
            .map_err(|error| DatabaseError::Write {
                error,
                table: T::NAME,
                key: Box::from(key.as_ref()),
            })
    }

    /// Puts a key/value pair into the database. The cursor will be positioned at the new data item,
    /// or on failure, usually near it.
    pub fn insert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);

        libmdbx::Cursor::put(
            &mut self.inner,
            key.as_ref(),
            value.as_ref(),
            WriteFlags::NO_OVERWRITE,
        )
        .map_err(|error| DatabaseError::Write {
            error,
            table: T::NAME,
            key: Box::from(key.as_ref()),
        })
    }

    /// Appends the data to the end of the table. Consequently, the append operation
    /// will fail if the inserted key is less than the last table key
    pub fn append(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);

        libmdbx::Cursor::put(&mut self.inner, key.as_ref(), value.as_ref(), WriteFlags::APPEND)
            .map_err(|error| DatabaseError::Write {
                error,
                table: T::NAME,
                key: Box::from(key.as_ref()),
            })
    }

    /// Deletes the current key/value pair.
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        libmdbx::Cursor::del(&mut self.inner, WriteFlags::CURRENT).map_err(DatabaseError::Delete)
    }
}

impl<T: DupSort> Cursor<RW, T> {
    /// Deletes all values for the current key.
    ///
    /// This will delete all values for the current duplicate key of a `DUPSORT` table, including
    /// the current item.
    pub fn delete_current_duplicates(&mut self) -> Result<(), DatabaseError> {
        libmdbx::Cursor::del(&mut self.inner, WriteFlags::NO_DUP_DATA)
            .map_err(DatabaseError::Delete)
    }

    pub fn append_dup(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);

        libmdbx::Cursor::put(&mut self.inner, key.as_ref(), value.as_ref(), WriteFlags::APPEND_DUP)
            .map_err(|error| DatabaseError::Write {
                error,
                table: T::NAME,
                key: Box::from(key.as_ref()),
            })
    }
}

/// A key-value pair coming from an iterator.
///
/// The `Result` represents that the operation might fail, while the `Option` represents whether or
/// not there is an entry.
pub type IterPairResult<T> = Option<Result<KeyValue<T>, DatabaseError>>;

/// Provides an iterator to a `Cursor` when handling `Table`.
#[derive(Debug)]
pub struct Walker<'c, K: TransactionKind, T: Table> {
    /// Cursor to be used to walk through the table.
    cursor: &'c mut Cursor<K, T>,
    /// Initial position of the dup walker. The value (key/value pair)  where to start the walk.
    start: IterPairResult<T>,
}

impl<'c, K, T> Walker<'c, K, T>
where
    K: TransactionKind,
    T: Table,
{
    /// Create a new [`Walker`] from a [`Cursor`] and a [`IterPairResult`].
    pub(super) fn new(cursor: &'c mut Cursor<K, T>, start: IterPairResult<T>) -> Self {
        Self { cursor, start }
    }
}

impl<T: Table> Walker<'_, RW, T> {
    /// Delete the `key/value` pair item at the current position of the walker.
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.cursor.delete_current()
    }
}

impl<K: TransactionKind, T: Table> std::iter::Iterator for Walker<'_, K, T> {
    type Item = Result<KeyValue<T>, DatabaseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if let value @ Some(_) = self.start.take() { value } else { self.cursor.next().transpose() }
    }
}

/// A cursor iterator for `DUPSORT` table.
///
/// Similar to [`Walker`], but for `DUPSORT` table.
#[derive(Debug)]
pub struct DupWalker<'c, K: TransactionKind, T: DupSort> {
    /// Cursor to be used to walk through the table.
    cursor: &'c mut Cursor<K, T>,
    /// Initial position of the dup walker. The value (key/value pair) where to start the walk.
    start: IterPairResult<T>,
}

impl<'c, K, T> DupWalker<'c, K, T>
where
    K: TransactionKind,
    T: DupSort,
{
    /// Creates a new [`DupWalker`] from a [`Cursor`] and a [`IterPairResult`].
    pub(super) fn new(cursor: &'c mut Cursor<K, T>, start: IterPairResult<T>) -> Self {
        Self { cursor, start }
    }
}

impl<T: DupSort> DupWalker<'_, RW, T> {
    /// Delete the item at the current position of the walker.
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.cursor.delete_current()
    }
}

impl<K: TransactionKind, T: DupSort> std::iter::Iterator for DupWalker<'_, K, T> {
    type Item = Result<KeyValue<T>, DatabaseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if let value @ Some(_) = self.start.take() {
            value
        } else {
            self.cursor.next_dup().transpose()
        }
    }
}
