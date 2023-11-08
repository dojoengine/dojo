use std::io::Read;

use flate2::Compression;

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
    T: serde::Serialize,
{
    type Encoded = Vec<u8>;
    fn encode(self) -> Self::Encoded {
        bincode::serialize(&self).expect("valid encoding")
    }
}

impl<T> Decode for T
where
    T: for<'a> serde::Deserialize<'a>,
{
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        bincode::deserialize(bytes.as_ref()).map_err(|e| CodecError::Decode(e.to_string()))
    }
}

impl<T> Compress for T
where
    T: Encode + serde::Serialize,
    <T as Encode>::Encoded: AsRef<[u8]>,
{
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        let mut compressed = Vec::new();
        flate2::read::DeflateEncoder::new(Encode::encode(self).as_ref(), Compression::best())
            .read_to_end(&mut compressed)
            .unwrap();
        compressed
    }
}

impl<T> Decompress for T
where
    T: Decode + for<'a> serde::Deserialize<'a>,
{
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let mut bin = Vec::new();
        flate2::read::DeflateDecoder::new(bytes.as_ref())
            .read_to_end(&mut bin)
            .map_err(|e| CodecError::Decompress(e.to_string()))?;
        Decode::decode(bin)
    }
}
