use std::borrow::Cow;
use std::path::Path;

use crate::codecs::{Decode, Decompress};
use crate::error::DatabaseError;
use crate::mdbx::tables::Table;

/// Returns the default page size that can be used in this OS.
pub(crate) fn default_page_size() -> usize {
    let os_page_size = page_size::get();
    // source: https://gitflic.ru/project/erthink/libmdbx/blob?file=mdbx.h#line-num-821
    let libmdbx_max_page_size = 0x10000;
    // May lead to errors if it's reduced further because of the potential size of the
    // data.
    let min_page_size = 4096;
    os_page_size.clamp(min_page_size, libmdbx_max_page_size)
}

/// Check if a db is empty. It does not provide any information on the
/// validity of the data in it. We consider a database as non empty when it's a non empty directory.
pub fn is_database_empty<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    if !path.exists() {
        true
    } else if let Ok(dir) = path.read_dir() {
        dir.count() == 0
    } else {
        true
    }
}

/// A key-value pair for table `T`.
pub type KeyValue<T> = (<T as Table>::Key, <T as Table>::Value);

/// Helper function to decode a `(key, value)` pair.
pub(crate) fn decoder<'a, T: Table>(
    kv: (Cow<'a, [u8]>, Cow<'a, [u8]>),
) -> Result<(T::Key, T::Value), DatabaseError>
where
    T::Key: Decode,
    T::Value: Decompress,
{
    let key = match kv.0 {
        Cow::Borrowed(k) => Decode::decode(k)?,
        Cow::Owned(k) => Decode::decode(k)?,
    };
    let value = match kv.1 {
        Cow::Borrowed(v) => Decompress::decompress(v)?,
        Cow::Owned(v) => Decompress::decompress(v)?,
    };
    Ok((key, value))
}

/// Helper function to decode only a value from a `(key, value)` pair.
pub(crate) fn decode_value<'a, T>(
    kv: (Cow<'a, [u8]>, Cow<'a, [u8]>),
) -> Result<T::Value, DatabaseError>
where
    T: Table,
{
    Ok(match kv.1 {
        Cow::Borrowed(v) => Decompress::decompress(v)?,
        Cow::Owned(v) => Decompress::decompress(v)?,
    })
}

/// Helper function to decode a value. It can be a key or subkey.
pub(crate) fn decode_one<T>(value: Cow<'_, [u8]>) -> Result<T::Value, DatabaseError>
where
    T: Table,
{
    Ok(match value {
        Cow::Borrowed(v) => Decompress::decompress(v)?,
        Cow::Owned(v) => Decompress::decompress(v)?,
    })
}
