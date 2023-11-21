//! Cursor wrapper for libmdbx-sys.

use std::marker::PhantomData;

use libmdbx::{self, TransactionKind, WriteFlags, RW};

use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::tables::{DupSort, Table};
use crate::utils::{decode_one, decode_value, KeyValue};

/// Cursor wrapper to access KV items.
#[derive(Debug)]
pub struct Cursor<'tx, K: TransactionKind, T: Table> {
    /// Inner `libmdbx` cursor.
    pub(crate) inner: libmdbx::Cursor<'tx, K>,
    /// Phantom data to enforce encoding/decoding.
    _dbi: PhantomData<T>,
}

impl<'tx, K: TransactionKind, T: Table> Cursor<'tx, K, T> {
    pub(crate) fn new(inner: libmdbx::Cursor<'tx, K>) -> Self {
        Self { inner, _dbi: PhantomData }
    }
}

/// Takes `(key, value)` from the database and decodes it appropriately.
#[macro_export]
macro_rules! decode {
    ($v:expr) => {
        $v.map_err($crate::error::DatabaseError::Read)?.map($crate::utils::decoder::<T>).transpose()
    };
}

#[allow(clippy::should_implement_trait)]
impl<K: TransactionKind, T: DupSort> Cursor<'_, K, T> {
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

    pub fn next(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next(&mut self.inner))
    }

    pub fn prev(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::prev(&mut self.inner))
    }

    pub fn last(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::last(&mut self.inner))
    }

    pub fn current(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::get_current(&mut self.inner))
    }

    /// Returns the next `(key, value)` pair of a DUPSORT table.
    pub fn next_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_dup(&mut self.inner))
    }

    /// Returns the next `(key, value)` pair skipping the duplicates.
    pub fn next_no_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_nodup(&mut self.inner))
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

impl<T: Table> Cursor<'_, RW, T> {
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

impl<T: DupSort> Cursor<'_, RW, T> {
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
