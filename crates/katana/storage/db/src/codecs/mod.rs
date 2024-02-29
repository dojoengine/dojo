#[cfg(feature = "postcard")]
pub mod postcard;

use katana_primitives::block::FinalityStatus;
use katana_primitives::class::FlattenedSierraClass;
use katana_primitives::contract::ContractAddress;
use katana_primitives::FieldElement;

use crate::error::CodecError;

/// A trait for encoding the key of a table.
pub trait Encode {
    type Encoded: AsRef<[u8]> + Into<Vec<u8>>;
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

macro_rules! impl_encode_and_decode_for_uints {
    ($($ty:ty),*) => {
        $(
            impl Encode for $ty {
                type Encoded = [u8; std::mem::size_of::<$ty>()];
                fn encode(self) -> Self::Encoded {
                    self.to_be_bytes()
                }
            }

            impl Decode for $ty {
                fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
                    let mut buf = [0u8; std::mem::size_of::<$ty>()];
                    buf.copy_from_slice(bytes.as_ref());
                    Ok(Self::from_be_bytes(buf))
                }
            }
        )*
    }
}

macro_rules! impl_encode_and_decode_for_felts {
    ($($ty:ty),*) => {
        $(
            impl Encode for $ty {
                type Encoded = [u8; 32];
                fn encode(self) -> Self::Encoded {
                    self.to_bytes_be()
                }
            }

            impl Decode for $ty {
                fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
                    let felt = FieldElement::from_byte_slice_be(bytes.as_ref());
                    Ok(felt.map_err(|e| CodecError::Decode(e.to_string()))?.into())
                }
            }
        )*
    }
}

impl_encode_and_decode_for_uints!(u64);
impl_encode_and_decode_for_felts!(FieldElement, ContractAddress);

impl Compress for FlattenedSierraClass {
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        serde_json::to_vec(&self).unwrap()
    }
}

impl Decompress for FlattenedSierraClass {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        serde_json::from_slice(bytes.as_ref()).map_err(|e| CodecError::Decode(e.to_string()))
    }
}

impl Compress for FinalityStatus {
    type Compressed = [u8; 1];
    fn compress(self) -> Self::Compressed {
        [self as u8]
    }
}

impl Decompress for FinalityStatus {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        match bytes.as_ref().first() {
            Some(0) => Ok(FinalityStatus::AcceptedOnL2),
            Some(1) => Ok(FinalityStatus::AcceptedOnL1),
            _ => Err(CodecError::Decode("Invalid status".into())),
        }
    }
}
