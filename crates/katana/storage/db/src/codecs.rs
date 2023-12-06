use serde::{Deserialize, Serialize};

use crate::error::CodecError;

/// A trait for encoding the key of a table.
pub trait Encode {
    type Encoded: AsRef<[u8]>;
    fn encode(self) -> Self::Encoded;
}

pub trait Decode: Sized {
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError>;
}

/// A trait for compressing data that are stored in the db.
pub trait Compress {
    type Compressed: AsRef<[u8]>;
    fn compress(self) -> Self::Compressed;
}

/// A trait for decompressing data that are read from the db.
pub trait Decompress: Sized {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError>;
}

impl<T> Encode for T
where
    T: Serialize,
{
    type Encoded = Vec<u8>;
    fn encode(self) -> Self::Encoded {
        serde_json::to_vec(&self).unwrap()
    }
}

impl<T> Decode for T
where
    T: for<'de> Deserialize<'de>,
{
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        serde_json::from_slice(bytes.as_ref()).map_err(|e| CodecError::Decode(e.to_string()))
    }
}

impl<T> Compress for T
where
    T: Serialize,
{
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        self.encode()
    }
}

impl<T> Decompress for T
where
    T: for<'de> Deserialize<'de>,
{
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        Self::decode(bytes)
    }
}
