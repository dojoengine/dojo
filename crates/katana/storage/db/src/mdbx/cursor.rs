//! Cursor wrapper for libmdbx-sys.

use std::borrow::Cow;
use std::marker::PhantomData;

use libmdbx::{self, TransactionKind, WriteFlags, RW};

use crate::abstraction::{
    DbCursor, DbCursorMut, DbDupSortCursor, DbDupSortCursorMut, DupWalker, Walker,
};
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

impl<K, T> DbCursor<T> for Cursor<K, T>
where
    K: TransactionKind,
    T: Table,
{
    fn first(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::first(&mut self.inner))
    }

    fn current(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::get_current(&mut self.inner))
    }

    fn next(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next(&mut self.inner))
    }

    fn prev(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::prev(&mut self.inner))
    }

    fn last(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::last(&mut self.inner))
    }

    fn set(&mut self, key: <T as Table>::Key) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::set_key(&mut self.inner, key.encode().as_ref()))
    }

    fn seek(&mut self, key: <T as Table>::Key) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::set_range(&mut self.inner, key.encode().as_ref()))
    }

    fn walk(&mut self, start_key: Option<T::Key>) -> Result<Walker<'_, T, Self>, DatabaseError> {
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

impl<K, T> DbDupSortCursor<T> for Cursor<K, T>
where
    K: TransactionKind,
    T: DupSort,
{
    fn next_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_dup(&mut self.inner))
    }

    fn next_dup_val(&mut self) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        libmdbx::Cursor::next_dup(&mut self.inner)
            .map_err(DatabaseError::Read)?
            .map(decode_value::<T>)
            .transpose()
    }

    fn next_no_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(libmdbx::Cursor::next_nodup(&mut self.inner))
    }

    fn seek_by_key_subkey(
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

    fn walk_dup(
        &mut self,
        key: Option<T::Key>,
        subkey: Option<T::SubKey>,
    ) -> Result<Option<DupWalker<'_, T, Self>>, DatabaseError> {
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

impl<T> DbCursorMut<T> for Cursor<RW, T>
where
    T: Table,
{
    fn upsert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);

        libmdbx::Cursor::put(&mut self.inner, key.as_ref(), value.as_ref(), WriteFlags::UPSERT)
            .map_err(|error| DatabaseError::Write {
                error,
                table: T::NAME,
                key: Box::from(key.as_ref()),
            })
    }

    fn insert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
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

    fn append(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);

        libmdbx::Cursor::put(&mut self.inner, key.as_ref(), value.as_ref(), WriteFlags::APPEND)
            .map_err(|error| DatabaseError::Write {
                error,
                table: T::NAME,
                key: Box::from(key.as_ref()),
            })
    }

    fn delete_current(&mut self) -> Result<(), DatabaseError> {
        libmdbx::Cursor::del(&mut self.inner, WriteFlags::CURRENT).map_err(DatabaseError::Delete)
    }
}

impl<T> DbDupSortCursorMut<T> for Cursor<RW, T>
where
    T: DupSort,
{
    fn delete_current_duplicates(&mut self) -> Result<(), DatabaseError> {
        libmdbx::Cursor::del(&mut self.inner, WriteFlags::NO_DUP_DATA)
            .map_err(DatabaseError::Delete)
    }

    fn append_dup(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
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
