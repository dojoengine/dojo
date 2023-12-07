use std::array::TryFromSliceError;

use katana_primitives::block::{FinalityStatus, Header};
use katana_primitives::contract::{ContractAddress, GenericContractInfo, SierraClass};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::Tx;
use katana_primitives::FieldElement;
use serde::{Deserialize, Serialize};

use crate::error::CodecError;
use crate::models::block::StoredBlockBodyIndices;
use crate::models::contract::StoredContractClass;

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

// WARNING!
//
// BELOW HERE ARE REALLY NAIVE IMPLEMENTATIONS OF THE ABOVE TRAITS JUST TO GET A WORKING POC
// QUICKLY, FOLLOWING PATH OF LEAST RESISTANCE. THEY SHOULD BE REPLACED WITH SOMETHING MORE
// EFFICIENT.

macro_rules! impl_encode_decode_for_serde_types {
    ($($ty:ty),*) => {
        $(
            impl Encode for $ty {
                type Encoded = Vec<u8>;
                fn encode(self) -> Self::Encoded {
                    serde_json::to_vec(&self).unwrap()
                }
            }

            impl Decode for $ty {
                fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
                    serde_json::from_slice(bytes.as_ref()).map_err(|e| CodecError::Decode(e.to_string()))
                }
            }
        )*
    };
}

macro_rules! impl_encode_decode_for_uints {
    ($($ty:ty),+) => {
        $(
            impl Encode for $ty {
                type Encoded = [u8; std::mem::size_of::<$ty>()];
                fn encode(self) -> Self::Encoded {
                    self.to_be_bytes()
                }
            }

            impl Decode for $ty {
                fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
                    let bytes: [u8; std::mem::size_of::<$ty>()] = bytes
                        .as_ref()
                        .try_into()
                        .map_err(|e: TryFromSliceError| CodecError::Decode(e.to_string()))?;
                    Ok(<$ty>::from_be_bytes(bytes))
                }
            }
        )+
    };
}

impl_encode_decode_for_uints!(u64);

impl_encode_decode_for_serde_types!(
    Header,
    FinalityStatus,
    StoredBlockBodyIndices,
    Tx,
    Receipt,
    StoredContractClass,
    SierraClass,
    GenericContractInfo
);

macro_rules! impl_encode_decode_for_felt {
    ($($ty:ty),+) => {
        $(
            impl Encode for $ty {
                type Encoded = [u8; 32];
                fn encode(self) -> Self::Encoded {
                    self.to_bytes_be()
                }
            }

            impl Decode for $ty {
                fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
                    FieldElement::from_byte_slice_be(bytes.as_ref())
                        .map(|fe| fe.into())
                        .map_err(|e| CodecError::Decode(e.to_string()))
                }
            }

        )+
    };
}

impl_encode_decode_for_felt!(FieldElement, ContractAddress);

impl<T> Encode for Vec<T>
where
    T: Serialize,
{
    type Encoded = Vec<u8>;
    fn encode(self) -> Self::Encoded {
        serde_json::to_vec(&self).unwrap()
    }
}

impl<T> Decode for Vec<T>
where
    T: for<'a> Deserialize<'a>,
{
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        serde_json::from_slice(bytes.as_ref()).map_err(|e| CodecError::Decode(e.to_string()))
    }
}

impl<T> Compress for T
where
    T: Encode,
    <T as Encode>::Encoded: Into<Vec<u8>>,
{
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        self.encode().into()
    }
}

impl<T> Decompress for T
where
    T: Decode,
{
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        Self::decode(bytes)
    }
}
