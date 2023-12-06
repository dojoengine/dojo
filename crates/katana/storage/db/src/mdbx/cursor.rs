//! Cursor wrapper for libmdbx-sys.

use std::marker::PhantomData;

use libmdbx::{self, TransactionKind, WriteFlags, RW};

use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::tables::{DupSort, Table};
use crate::utils::{decode_one, decode_value, decoder, KeyValue};

/// Cursor for navigating the items within a database.
#[derive(Debug)]
pub struct Cursor<K: TransactionKind, T: Table> {
    /// Inner `libmdbx` cursor.
    pub(crate) inner: libmdbx::Cursor<K>,
    /// Phantom data to enforce encoding/decoding.
    _dbi: PhantomData<T>,
}

impl<K: TransactionKind, T: Table> Cursor<K, T> {
    pub(crate) fn new(inner: libmdbx::Cursor<K>) -> Self {
        Self { inner, _dbi: PhantomData }
    }
}

/// Takes `(key, value)` from the database and decodes it appropriately.
macro_rules! decode {
    ($v:expr) => {
        $v.map_err($crate::error::DatabaseError::Read)?.map($crate::utils::decoder::<T>).transpose()
    };
}

impl<K: TransactionKind, T: Table> Cursor<K, T> {
    /// Positions the cursor at the first entry in the table, returning the value.
    pub fn first(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::first(&mut self.inner))
    }

    pub fn seek_exact(
        &mut self,
        key: <T as Table>::Key,
    ) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::set_key(&mut self.inner, key.encode().as_ref()))
    }

    pub fn seek(&mut self, key: <T as Table>::Key) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::set_range(&mut self.inner, key.encode().as_ref()))
    }

    /// Position the cursor at the next KV pair, returning the value.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next(&mut self.inner))
    }

    /// Position the cursor at the previous KV pair, returning the value.
    pub fn prev(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::prev(&mut self.inner))
    }

    pub fn last(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::last(&mut self.inner))
    }

    pub fn current(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::get_current(&mut self.inner))
    }

    /// Returns the next `(key, value)` pair skipping the duplicates.
    pub fn next_no_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_nodup(&mut self.inner))
    }

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
    /// Returns the next `(key, value)` pair of a DUPSORT table.
    pub fn next_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_dup(&mut self.inner))
    }

    /// Returns the next `value` of a duplicate `key`.
    pub fn next_dup_val(&mut self) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        libmdbx::Cursor::next_dup(&mut self.inner)
            .map_err(DatabaseError::Read)?
            .map(decode_value::<T>)
            .transpose()
    }

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
}

impl<T: Table> Cursor<RW, T> {
    /// Database operation that will update an existing row if a specified value already
    /// exists in a table, and insert a new row if the specified value doesn't already exist
    ///
    /// For a DUPSORT table, `upsert` will not actually update-or-insert. If the key already exists,
    /// it will append the value to the subkey, even if the subkeys are the same. So if you want
    /// to properly upsert, you'll need to `seek_exact` & `delete_current` if the key+subkey was
    /// found, before calling `upsert`.
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

    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        libmdbx::Cursor::del(&mut self.inner, WriteFlags::CURRENT).map_err(DatabaseError::Delete)
    }
}

impl<T: DupSort> Cursor<RW, T> {
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
/// not there is another entry.
pub type IterPairResult<T> = Option<Result<KeyValue<T>, DatabaseError>>;

/// Provides an iterator to `Cursor` when handling `Table`.
///
/// Reason why we have two lifetimes is to distinguish between `'cursor` lifetime
/// and inherited `'tx` lifetime. If there is only one, rust would short circle
/// the Cursor lifetime and it wouldn't be possible to use Walker.
pub struct Walker<'c, K: TransactionKind, T: Table> {
    /// Cursor to be used to walk through the table.
    cursor: &'c mut Cursor<K, T>,
    /// `(key, value)` where to start the walk.
    start: IterPairResult<T>,
}

impl<K, T> std::fmt::Debug for Walker<'_, K, T>
where
    K: TransactionKind,
    T: Table + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Walker").field("cursor", &self.cursor).field("start", &self.start).finish()
    }
}

impl<'c, K, T> Walker<'c, K, T>
where
    K: TransactionKind,
    T: Table,
{
    /// Create a new [`Walker`] from a [`Cursor`] and a [`IterPairResult`].
    pub fn new(cursor: &'c mut Cursor<K, T>, start: IterPairResult<T>) -> Self {
        Self { cursor, start }
    }
}

impl<'c, T> Walker<'c, RW, T>
where
    T: Table,
{
    /// Delete current item that walker points to.
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
