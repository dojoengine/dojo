use katana_primitives::contract::{StorageKey, StorageValue};

use crate::codecs::{Compress, Decompress};
use crate::error::CodecError;

/// Represents a contract storage entry.
///
/// `key` is the subkey for the dupsort table.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct StorageEntry {
    /// The storage key.
    pub key: StorageKey,
    /// The storage value.
    pub value: StorageValue,
}

impl Compress for StorageEntry {
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.key.to_bytes_be());
        buf.extend_from_slice(&self.value.compress());
        buf
    }
}

impl Decompress for StorageEntry {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, crate::error::CodecError> {
        let bytes = bytes.as_ref();
        let key = StorageKey::from_byte_slice_be(&bytes[0..32])
            .map_err(|e| CodecError::Decompress(e.to_string()))?;
        let value = StorageValue::decompress(&bytes[32..])?;
        Ok(Self { key, value })
    }
}
