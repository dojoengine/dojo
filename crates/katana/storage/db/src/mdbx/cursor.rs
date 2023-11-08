//! Cursor wrapper for libmdbx-sys.

use std::marker::PhantomData;

use libmdbx::{self, TransactionKind, WriteFlags, RO, RW};

use super::tables::{DupSort, Table};
use crate::codecs::{Compress, Encode};
use crate::error::DatabaseError;
use crate::utils::{decode_one, decode_value, KeyValue};

/// Read only Cursor.
pub type CursorRO<'tx, T> = Cursor<'tx, RO, T>;
/// Read write cursor.
pub type CursorRW<'tx, T> = Cursor<'tx, RW, T>;

/// Cursor wrapper to access KV items.
#[derive(Debug)]
pub struct Cursor<'tx, K: TransactionKind, T: Table> {
    /// Inner `libmdbx` cursor.
    pub(crate) inner: libmdbx::Cursor<'tx, K>,
    /// Cache buffer that receives compressed values.
    buf: Vec<u8>,
    /// Phantom data to enforce encoding/decoding.
    _dbi: PhantomData<T>,
}

impl<'tx, K: TransactionKind, T: Table> Cursor<'tx, K, T> {
    pub(crate) fn new(inner: libmdbx::Cursor<'tx, K>) -> Self {
        Self { inner, buf: Vec::new(), _dbi: PhantomData }
    }
}

/// Takes `(key, value)` from the database and decodes it appropriately.
#[macro_export]
macro_rules! decode {
    ($v:expr) => {
        $v.map_err($crate::error::DatabaseError::Read)?.map($crate::utils::decoder::<T>).transpose()
    };
}

impl<K: TransactionKind, T: Table> Cursor<'_, K, T> {
    // fn walk(&mut self, start_key: Option<T::Key>) -> Result<Walker<'_, T, Self>, DatabaseError>
    // where
    //     Self: Sized,
    // {
    //     let start = if let Some(start_key) = start_key {
    //         self.inner
    //             .set_range(start_key.encode().as_ref())
    //             .map_err(|e| DatabaseError::Read(e.into()))?
    //             .map(decoder::<T>)
    //     } else {
    //         self.first().transpose()
    //     };

    //     Ok(Walker::new(self, start))
    // }

    // fn walk_range(
    //     &mut self,
    //     range: impl RangeBounds<T::Key>,
    // ) -> Result<RangeWalker<'_, T, Self>, DatabaseError>
    // where
    //     Self: Sized,
    // {
    //     let start = match range.start_bound().cloned() {
    //         Bound::Included(key) => self.inner.set_range(key.encode().as_ref()),
    //         Bound::Excluded(_key) => {
    //             unreachable!("Rust doesn't allow for Bound::Excluded in starting bounds");
    //         }
    //         Bound::Unbounded => self.inner.first(),
    //     }
    //     .map_err(|e| DatabaseError::Read(e.into()))?
    //     .map(decoder::<T>);

    //     Ok(RangeWalker::new(self, start, range.end_bound().cloned()))
    // }

    // fn walk_back(
    //     &mut self,
    //     start_key: Option<T::Key>,
    // ) -> Result<ReverseWalker<'_, T, Self>, DatabaseError>
    // where
    //     Self: Sized,
    // {
    //     let start = if let Some(start_key) = start_key {
    //         decode!(self.inner.set_range(start_key.encode().as_ref()))
    //     } else {
    //         self.last()
    //     }
    //     .transpose();

    //     Ok(ReverseWalker::new(self, start))
    // }
}

impl<K: TransactionKind, T: DupSort> Cursor<'_, K, T> {
    fn first(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.first())
    }

    fn seek_exact(&mut self, key: <T as Table>::Key) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.set_key(key.encode().as_ref()))
    }

    fn seek(&mut self, key: <T as Table>::Key) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.set_range(key.encode().as_ref()))
    }

    fn next(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.next())
    }

    fn prev(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.prev())
    }

    fn last(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.last())
    }

    fn current(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.get_current())
    }

    /// Returns the next `(key, value)` pair of a DUPSORT table.
    fn next_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.next_dup())
    }

    /// Returns the next `(key, value)` pair skipping the duplicates.
    fn next_no_dup(&mut self) -> Result<Option<KeyValue<T>>, DatabaseError> {
        decode!(self.inner.next_nodup())
    }

    /// Returns the next `value` of a duplicate `key`.
    fn next_dup_val(&mut self) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        self.inner
            .next_dup()
            .map_err(|e| DatabaseError::Read(e.into()))?
            .map(decode_value::<T>)
            .transpose()
    }

    fn seek_by_key_subkey(
        &mut self,
        key: <T as Table>::Key,
        subkey: <T as DupSort>::SubKey,
    ) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        self.inner
            .get_both_range(key.encode().as_ref(), subkey.encode().as_ref())
            .map_err(|e| DatabaseError::Read(e.into()))?
            .map(decode_one::<T>)
            .transpose()
    }

    // /// Depending on its arguments, returns an iterator starting at:
    // /// - Some(key), Some(subkey): a `key` item whose data is >= than `subkey`
    // /// - Some(key), None: first item of a specified `key`
    // /// - None, Some(subkey): like first case, but in the first key
    // /// - None, None: first item in the table
    // /// of a DUPSORT table.
    // fn walk_dup(
    //     &mut self,
    //     key: Option<T::Key>,
    //     subkey: Option<T::SubKey>,
    // ) -> Result<DupWalker<'_, T, Self>, DatabaseError> {
    //     let start = match (key, subkey) {
    //         (Some(key), Some(subkey)) => {
    //             // encode key and decode it after.
    //             let key = key.encode().as_ref().to_vec();

    //             self.inner
    //                 .get_both_range(key.as_ref(), subkey.encode().as_ref())
    //                 .map_err(|e| DatabaseError::Read(e.into()))?
    //                 .map(|val| decoder::<T>((Cow::Owned(key), val)))
    //         }
    //         (Some(key), None) => {
    //             let key = key.encode().as_ref().to_vec();

    //             self.inner
    //                 .set(key.as_ref())
    //                 .map_err(|e| DatabaseError::Read(e.into()))?
    //                 .map(|val| decoder::<T>((Cow::Owned(key), val)))
    //         }
    //         (None, Some(subkey)) => {
    //             if let Some((key, _)) = self.first()? {
    //                 let key = key.encode().as_ref().to_vec();

    //                 self.inner
    //                     .get_both_range(key.as_ref(), subkey.encode().as_ref())
    //                     .map_err(|e| DatabaseError::Read(e.into()))?
    //                     .map(|val| decoder::<T>((Cow::Owned(key), val)))
    //             } else {
    //                 let err_code = MDBXError::to_err_code(&MDBXError::NotFound);
    //                 Some(Err(DatabaseError::Read(err_code)))
    //             }
    //         }
    //         (None, None) => self.first().transpose(),
    //     };

    //     Ok(DupWalker::<'_, T, Self> { cursor: self, start })
    // }
}

impl<T: Table> Cursor<'_, RW, T> {
    /// Database operation that will update an existing row if a specified value already
    /// exists in a table, and insert a new row if the specified value doesn't already exist
    ///
    /// For a DUPSORT table, `upsert` will not actually update-or-insert. If the key already exists,
    /// it will append the value to the subkey, even if the subkeys are the same. So if you want
    /// to properly upsert, you'll need to `seek_exact` & `delete_current` if the key+subkey was
    /// found, before calling `upsert`.
    fn upsert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);
        self.inner.put(key.as_ref(), value.as_ref(), WriteFlags::UPSERT).map_err(|error| {
            DatabaseError::Write { error, table: T::NAME, key: Box::from(key.as_ref()) }
        })
    }

    fn insert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);
        self.inner.put(key.as_ref(), value.as_ref(), WriteFlags::NO_OVERWRITE).map_err(|error| {
            DatabaseError::Write { error, table: T::NAME, key: Box::from(key.as_ref()) }
        })
    }

    /// Appends the data to the end of the table. Consequently, the append operation
    /// will fail if the inserted key is less than the last table key
    fn append(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);
        self.inner.put(key.as_ref(), value.as_ref(), WriteFlags::APPEND).map_err(|error| {
            DatabaseError::Write { error, table: T::NAME, key: Box::from(key.as_ref()) }
        })
    }

    fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.inner.del(WriteFlags::CURRENT).map_err(|e| DatabaseError::Delete(e.into()))
    }
}

impl<T: DupSort> Cursor<'_, RW, T> {
    fn delete_current_duplicates(&mut self) -> Result<(), DatabaseError> {
        self.inner.del(WriteFlags::NO_DUP_DATA).map_err(DatabaseError::Delete)
    }

    fn append_dup(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = Encode::encode(key);
        let value = Compress::compress(value);
        self.inner.put(key.as_ref(), value.as_ref(), WriteFlags::APPEND_DUP).map_err(|error| {
            DatabaseError::Write { error, table: T::NAME, key: Box::from(key.as_ref()) }
        })
    }
}
